use std::fmt::Debug;
use std::sync::Arc;

use crate::commands::{deauthenticate::Deauthenticate, e2e::E2EDispatcher};
use crate::rpc::command::dispatcher::{DispatcherError, TakeableCommandDispatcher};
use crate::rpc::connection::Connection;
use crate::rpc::peer::Peer;
use crate::rpc::session::SessionReadError;
use crate::rpc::{command::handler::CommandHandler, server::RpcServer, session::Session};
use crate::transport::session_transport::SessionTransport;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, Credential, SpkiHash};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use tracing::debug;

fn forward_key() -> String {
    "forward".to_owned()
}

#[derive(Serialize, Deserialize, Debug, thiserror::Error)]
pub enum ForwardError {
    #[error("The requested target is not currently available")]
    TargetNotConnected,
    #[error("deauthentication failed")]
    DeauthFailure,
}

pub struct ForwardHandler {
    server: Arc<RpcServer>,
}

impl ForwardHandler {
    pub fn new(server: Arc<RpcServer>) -> Self {
        Self { server }
    }
}

#[async_trait]
impl CommandHandler for ForwardHandler {
    type Request = SpkiHash;

    fn key() -> String {
        forward_key()
    }

    async fn handle(
        &self,
        session: &mut Session,
        target: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        debug!("received forward request to {:?}", target);

        match self.server.open_session_with(target).await {
            Ok(forward_session) => match forward_session.dispatch(Deauthenticate).await {
                Ok(mut forward_session) => {
                    debug!("deauth successful, forwarding session");

                    session
                        .write_object::<Result<(), ForwardError>>(&Ok(()))
                        .await?;

                    let transport1 = session.borrow_transport();
                    let transport2 = forward_session.borrow_transport();

                    if let Some(result) = cancel
                        .run_until_cancelled(tokio::io::copy_bidirectional(transport1, transport2))
                        .await
                    {
                        result?;
                    }

                    let _ = transport1.shutdown().await;
                    let _ = transport2.shutdown().await;

                    Ok(())
                }
                Err(err) => {
                    session
                        .write_object::<Result<(), ForwardError>>(&Err(ForwardError::DeauthFailure))
                        .await?;

                    Err(err.into())
                }
            },
            // TODO: check and return the actual error
            Err(err) => {
                tracing::error!("error during session forwarding: {err}");
                session
                    .write_object::<Result<(), ForwardError>>(&Err(
                        ForwardError::TargetNotConnected,
                    ))
                    .await?;
                Err(err)
            }
        }
    }
}

pub struct ForwardDispatcher {
    target: SpkiHash,
}

impl ForwardDispatcher {
    pub fn new(target: &Certificate) -> Self {
        Self {
            target: target.spki_hash().clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ForwardDispatchError {
    #[error("error reading forward signal: {0}")]
    AwaitForwardError(SessionReadError),
    #[error("received forward error from server: {0}")]
    ForwardError(#[from] ForwardError),
}

impl TakeableCommandDispatcher for ForwardDispatcher {
    type Output = (Box<dyn SessionTransport>);
    type InnerError = ForwardDispatchError;

    type Request = SpkiHash;

    fn key() -> String {
        forward_key()
    }

    fn get_request(&self) -> &Self::Request {
        &self.target
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
    ) -> Result<Self::Output, DispatcherError<Self::InnerError>> {
        if let Some(mut session) = session.take() {
            session
                .read_object::<Result<(), ForwardError>>()
                .await
                .map_err(ForwardDispatchError::AwaitForwardError)?
                .map_err(ForwardDispatchError::ForwardError)?;

            let (transport, _) = session.destructure();

            Ok(transport)
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}

#[derive(Clone)]
pub struct ForwardConnection<T> {
    connection: T,
    credentials: Credential,
    target: Certificate,
    as_peer: Peer,
}

impl<T> ForwardConnection<T> {
    pub fn new(base_connection: T, credentials: Credential, target: Certificate) -> Self {
        Self {
            connection: base_connection,
            credentials,
            as_peer: Peer::Certificate(target.clone()),
            target,
        }
    }
}

#[async_trait]
impl<T> Connection for ForwardConnection<T>
where
    T: Connection + Send,
{
    async fn open_raw_session(&self) -> Result<Box<dyn SessionTransport>> {
        let dispatcher = ForwardDispatcher::new(&self.target);
        let transport = self.connection.dispatch(dispatcher).await?;

        let unencrypted = Session::new(transport, self.peer().clone());

        let dispatcher = E2EDispatcher {
            peer: self.target.clone(),
            credentials: &self.credentials,
        };

        let transport = unencrypted.dispatch(dispatcher).await?;

        Ok(transport)
    }

    fn peer(&self) -> &Peer {
        &self.as_peer
    }

    async fn closed(&self) {}
}
