use std::error::Error;
use std::fmt::{Debug, Display};
use std::sync::Arc;

use crate::commands::{deauthenticate::Deauthenticate, e2e::E2EDispatcher};
use crate::rpc::command::dispatcher::{DispatcherError, TakeableCommandDispatcher};
use crate::rpc::connection::Connection;
use crate::rpc::peer::Peer;
use crate::rpc::session::{SessionDispatchError, SessionReadError};
use crate::rpc::{command::handler::CommandHandler, server::RpcServer, session::Session};
use crate::transport::combined_transport::CombinedTransport;
use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, PermCredentials};
use tokio::io::AsyncWriteExt;
use tokio::select;
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
    type Request = Certificate;

    fn key() -> String {
        forward_key()
    }

    #[must_use]
    async fn handle(
        &self,
        session: &mut Session,
        target: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        debug!("client requesting forward");

        debug!("received forward request to {:?}", target);

        match self.server.open_session_with(target).await {
            Ok(forward_session) => match forward_session.dispatch(Deauthenticate).await {
                Ok(forward_session) => {
                    debug!("deauth successful, forwarding session");

                    session
                        .write_object::<Result<(), ForwardError>>(&Ok(()))
                        .await?;

                    let (read1, write1) = session.borrow_transport();
                    let (read2, write2, _) = forward_session.destructure_transport();

                    let mut transport1 = CombinedTransport::new(read1, write1);
                    let mut transport2 = CombinedTransport::new(read2, write2);

                    tokio::io::copy_bidirectional(&mut transport1, &mut transport2).await?;

                    select! {
                        result = tokio::io::copy_bidirectional(&mut transport1, &mut transport2) => {
                            result?;
                        }
                        _ = cancel.cancelled() => {}
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

pub struct ForwardDispatcher<'a, T> {
    pub target: &'a Certificate,
    pub nested_dispatch: T,
}

#[derive(Debug, thiserror::Error)]
pub enum ForwardDispatchError<NestedError> {
    #[error("error reading forward signal: {0}")]
    AwaitForwardError(SessionReadError),
    #[error("received forward error from server: {0}")]
    ForwardError(#[from] ForwardError),
    #[error("error dispatching nested command: {0}")]
    NestedError(#[from] SessionDispatchError<NestedError>),
}

#[async_trait]
impl<'a, D> TakeableCommandDispatcher for ForwardDispatcher<'a, D>
where
    D: TakeableCommandDispatcher,
{
    type Output = D::Output;
    type InnerError = ForwardDispatchError<D::InnerError>;

    type Request = &'a Certificate;

    fn key() -> String {
        forward_key()
    }

    fn get_request(&self) -> Self::Request {
        self.target
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
        _target: Self::Request,
    ) -> Result<Self::Output, DispatcherError<ForwardDispatchError<D::InnerError>>> {
        if let Some(mut session) = session.take() {
            session
                .read_object::<Result<(), ForwardError>>()
                .await
                .map_err(ForwardDispatchError::AwaitForwardError)?
                .map_err(ForwardDispatchError::ForwardError)?;

            session
                .dispatch(self.nested_dispatch)
                .await
                .map_err(ForwardDispatchError::NestedError)
                .map_err(DispatcherError::Other)
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}

#[derive(Clone)]
pub struct ForwardConnection<T> {
    connection: T,
    credentials: PermCredentials,
    target: Certificate,
    as_peer: Peer,
}

impl<T> ForwardConnection<T> {
    pub fn new(base_connection: T, credentials: PermCredentials, target: Certificate) -> Self {
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
    async fn dispatch<D: TakeableCommandDispatcher>(&self, dispatcher: D) -> Result<D::Output>
    where
        D::InnerError: Display + 'static,
    {
        let dispatcher = ForwardDispatcher {
            target: &self.target,
            nested_dispatch: E2EDispatcher {
                peer: self.target.clone(),
                credentials: &self.credentials,
                nested_dispatch: dispatcher,
            },
        };

        self.connection
            .dispatch(dispatcher)
            .await
            .map_err(|err| anyhow!(err.to_string()))
    }

    fn peer(&self) -> &Peer {
        &self.as_peer
    }
}
