use std::error::Error;
use std::fmt::{Debug, Display};

use crate::rpc::command::HandlerCollection;
use crate::rpc::connection::{Connection, ConnectionBase};
use crate::rpc::peer::Peer;
use crate::rpc::{
    command::CommandHandler,
    server::RpcServer,
    session::{Session, SessionOpen},
};
use crate::transport::session_transport::SessionTransport;
use crate::transport::tls_transport::TlsTransport;
use crate::verifiers::exact::ExactServerVerification;
use crate::verifiers::skip_verify::SkipClientVerification;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, PermCredentials};
use tokio::io::AsyncWriteExt;
use tracing::{debug, error};

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

        base_session.request_session(e2e_key()).await?;

        let tls_transport = TlsTransport::client(
            base_session.extract_transport(),
            ExactServerVerification::new(&self.target),
            &self.credentials,
        )
        .await
        .map_err(|err| err.0)?;

        Ok(Box::new(tls_transport))
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

fn e2e_key() -> String {
    "e2e".into()
}

pub struct E2EHandler {
    credentials: PermCredentials,
    handler_collection: HandlerCollection,
}

impl E2EHandler {
    pub fn new(credentials: PermCredentials, handler_collection: HandlerCollection) -> Self {
        Self {
            credentials,
            handler_collection,
        }
    }
}

#[async_trait]
impl CommandHandler for E2EHandler {
    fn key(&self) -> String {
        e2e_key()
    }

    async fn handle(&self, session: &mut Session<SessionOpen>) -> anyhow::Result<()> {
        session
            .replace_transport(move |mut direct_transport| async move {
                if let Err(err) = direct_transport.flush().await {
                    error!("error replacing transport: {}", err);
                }
                let tls_transport = TlsTransport::server(
                    direct_transport,
                    // TODO: actually fucking verify the connecting peer
                    SkipClientVerification::new(),
                    &self.credentials,
                )
                .await;

                match tls_transport {
                    Ok(tls_transport) => Box::new(tls_transport),
                    Err(err) => err.1,
                }
            })
            .await;

        session.handle(self.handler_collection.clone()).await?;

        Ok(())
    }
}
