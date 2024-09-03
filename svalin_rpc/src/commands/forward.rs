use std::error::Error;
use std::fmt::{Debug, Display};

use crate::commands::e2e::E2EDispatcher;
use crate::rpc::command::dispatcher::{CommandDispatcher, TakeableCommandDispatcher};
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
}

impl Display for ForwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForwardError::TargetNotConnected => {
                write!(f, "Requested Target is not currently available")
            }
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
    fn key(&self) -> String {
        forward_key()
    }

    #[must_use]
    async fn handle(&self, session: &mut Session) -> anyhow::Result<()> {
        debug!("client requesting forward");

        let target: Certificate = session.read_object().await?;

        debug!("received forward request to {:?}", target);

        match self.server.open_raw_session_with(target).await {
            Ok((read2, write2)) => {
                session
                    .write_object::<Result<(), ForwardError>>(&Ok(()))
                    .await?;

                let (read1, write1) = session.borrow_transport();

                let mut transport1 = CombinedTransport::new(read1, write2);
                let mut transport2 = CombinedTransport::new(read2, write1);

                tokio::io::copy_bidirectional(&mut transport1, &mut transport2).await?;

                transport1.shutdown().await;
                transport2.shutdown().await;

                Ok(())
            }
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
    fn key(&self) -> String {
        forward_key()
    }
    async fn dispatch(self, session: &mut Option<Session>) -> Result<Self::Output> {
        if let Some(mut session) = session.take() {
            session.write_object(&self.target).await?;

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
