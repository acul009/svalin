use std::error::Error;
use std::fmt::{Debug, Display};

use crate::commands::{deauthenticate::Deauthenticate, e2e::E2EDispatcher};
use crate::rpc::command::dispatcher::TakeableCommandDispatcher;
use crate::rpc::connection::Connection;
use crate::rpc::{command::handler::CommandHandler, server::RpcServer, session::Session};
use crate::transport::combined_transport::CombinedTransport;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, PermCredentials};
use tokio::io::AsyncWriteExt;
use tracing::debug;

fn forward_key() -> String {
    "forward".to_owned()
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ForwardError {
    TargetNotConnected,
    DeauthFailure,
}

impl Display for ForwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForwardError::TargetNotConnected => {
                write!(f, "Requested Target is not currently available")
            }
            ForwardError::DeauthFailure => write!(f, "Deauthentication failed"),
        }
    }
}

impl Error for ForwardError {}

pub struct ForwardHandler {
    server: RpcServer,
}

impl ForwardHandler {
    pub fn new(server: RpcServer) -> Self {
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
    async fn handle(&self, session: &mut Session, target: Self::Request) -> anyhow::Result<()> {
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

                    let _ = transport1.shutdown().await;
                    let _ = transport2.shutdown().await;

                    Ok(())
                }
                Err(err) => {
                    session
                        .write_object::<Result<(), ForwardError>>(&Err(ForwardError::DeauthFailure))
                        .await?;

                    Err(err)
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

#[async_trait]
impl<'a, D> TakeableCommandDispatcher for ForwardDispatcher<'a, D>
where
    D: TakeableCommandDispatcher,
{
    type Output = D::Output;

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
    ) -> Result<Self::Output> {
        if let Some(mut session) = session.take() {
            session
                .read_object::<Result<(), ForwardError>>()
                .await
                .context("error during forward return signal")?
                .context("server sent error during forward request")?;

            session.dispatch(self.nested_dispatch).await
        } else {
            Err(anyhow!("tried dispatching command with None"))
        }
        // self.nested_dispatch.dispatch(session).await
    }
}

#[derive(Clone)]
pub struct ForwardConnection<T> {
    connection: T,
    credentials: PermCredentials,
    target: Certificate,
}

impl<T> ForwardConnection<T> {
    pub fn new(base_connection: T, credentials: PermCredentials, target: Certificate) -> Self {
        Self {
            connection: base_connection,
            credentials,
            target,
        }
    }
}

#[async_trait]
impl<T> Connection for ForwardConnection<T>
where
    T: Connection + Send,
{
    async fn dispatch<D: TakeableCommandDispatcher>(&self, dispatcher: D) -> Result<D::Output> {
        let dispatcher = ForwardDispatcher {
            target: &self.target,
            nested_dispatch: E2EDispatcher {
                peer: self.target.clone(),
                credentials: &self.credentials,
                nested_dispatch: dispatcher,
            },
        };

        self.connection.dispatch(dispatcher).await
    }
}
