use std::error::Error;
use std::fmt::{Debug, Display};

use crate as svalin_rpc;
use crate::rpc::connection::{Connection, ConnectionBase, DirectConnection};
use crate::rpc::peer::Peer;
use crate::rpc::{
    command::CommandHandler,
    server::RpcServer,
    session::{Session, SessionOpen},
};
use crate::transport::session_transport::SessionTransport;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_macros::rpc_dispatch;
use svalin_pki::Certificate;
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
    async fn handle(&self, session: &mut Session<SessionOpen>) -> anyhow::Result<()> {
        debug!("client requesting forward");

        let target: Certificate = session.read_object().await?;

        debug!("received forward request to {:?}", target);

        match self.server.open_raw_session_with(target).await {
            Ok(mut transport) => {
                session
                    .write_object::<Result<(), ForwardError>>(&Ok(()))
                    .await?;
                session.forward_transport(&mut transport).await?;
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

pub struct ForwardConnection<T> {
    connection: T,
    target: Certificate,
}

impl<T> ForwardConnection<T> {
    pub fn new(base_connection: T, target: Certificate) -> Result<Self> {
        Ok(Self {
            connection: base_connection,
            target: target,
        })
    }
}

#[async_trait]
impl<T> ConnectionBase for ForwardConnection<T>
where
    T: Connection,
{
    async fn open_raw_session(&self) -> Result<Box<dyn SessionTransport>> {
        debug!("opening base session");

        let mut base_session = self.connection.open_session(forward_key()).await?;

        debug!("requesting forward by server");

        base_session.write_object(&self.target).await?;

        base_session
            .read_object::<Result<(), ForwardError>>()
            .await
            .context("error during forward return signal")?
            .context("server sent error during forward request")?;

        Ok(base_session.extract_transport())
    }

    /// For the ForwardConnection we always return anonymous, since there is no
    /// E2E tunnel yet, we can't guarantee, that the server didn't intercept the
    /// session.
    fn peer(&self) -> &Peer {
        &Peer::Anonymous
    }

    async fn closed(&self) {
        self.connection.closed().await
    }
}

#[rpc_dispatch(forward_key())]
pub async fn forward(session: &mut Session<SessionOpen>) -> Result<()> {
    todo!()
}
