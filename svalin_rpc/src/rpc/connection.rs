use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use quinn::rustls::pki_types::CertificateDer;
use quinn::{RecvStream, SendStream, VarInt};
use svalin_pki::Certificate;
use tokio::task::JoinSet;
use tracing::field::debug;
use tracing::{debug, error};

use crate::transport::session_transport::SessionTransport;
use crate::{
    rpc::{
        command::HandlerCollection,
        session::SessionCreated,
        session::{Session, SessionOpen},
    },
    transport::combined_transport::CombinedTransport,
};

use super::peer::Peer;

#[async_trait]
pub trait Connection: ConnectionBase {
    async fn open_session(&self, command_key: String) -> Result<Session<SessionOpen>>;
}

#[async_trait]
impl<T> Connection for T
where
    T: ConnectionBase,
{
    async fn open_session(&self, command_key: String) -> Result<Session<SessionOpen>> {
        debug!("creating transport");

        let transport = self.open_raw_session().await?;

        debug!("transport created, pass to session");

        let session = Session::new(transport);

        debug!("requesting session");

        let session = session.request_session(command_key).await?;

        debug!("session request successful");

        Ok(session)
    }
}

#[async_trait]
pub trait ServeableConnectionBase: ConnectionBase {
    async fn accept_session(&self) -> Result<Session<SessionCreated>>;
}

#[async_trait]
pub trait ServeableConnection {
    async fn serve(&self, commands: Arc<HandlerCollection>) -> Result<()>;
}

#[async_trait]
impl<T> ServeableConnection for T
where
    T: ServeableConnectionBase,
{
    async fn serve(&self, commands: Arc<HandlerCollection>) -> Result<()> {
        debug!("waiting for incoming data stream");
        let mut open_sessions = JoinSet::<()>::new();

        loop {
            match self.accept_session().await {
                Ok(session) => {
                    let commands2 = commands.clone();
                    open_sessions.spawn(async move {
                        let res = session
                            .handle(commands2)
                            .await
                            .context("error handling session");
                        if let Err(e) = res {
                            // TODO: Actually handle Error
                            error!("{:?}", e);
                            #[cfg(test)]
                            {
                                panic!("{:?}", e);
                            }
                        }
                    });
                }
                Err(_err) => while open_sessions.join_next().await.is_some() {},
            }
        }
    }
}

#[async_trait]
pub trait ConnectionBase: Send + Sync {
    async fn open_raw_session(&self) -> Result<Box<dyn SessionTransport>>;

    fn peer(&self) -> &Peer;

    async fn closed(&self);
}

#[derive(Debug, Clone)]
pub struct DirectConnection {
    conn: quinn::Connection,
    peer: Peer,
}

#[async_trait]
impl crate::rpc::connection::ConnectionBase for DirectConnection {
    async fn open_raw_session(&self) -> Result<Box<dyn SessionTransport>> {
        let transport: CombinedTransport<SendStream, RecvStream> = self
            .conn
            .open_bi()
            .await
            .map_err(|err| anyhow!(err))?
            .into();

        Ok(Box::new(transport))
    }

    fn peer(&self) -> &Peer {
        &self.peer
    }

    async fn closed(&self) {
        // TODO: maybe return connection error from upstream?
        self.conn.closed().await;
    }
}

#[async_trait]
impl ServeableConnectionBase for DirectConnection {
    async fn accept_session(&self) -> Result<Session<SessionCreated>> {
        let transport: CombinedTransport<SendStream, RecvStream> = self
            .conn
            .accept_bi()
            .await
            .map_err(|err| anyhow!(err))?
            .into();

        debug("transport created");

        let session = Session::new(Box::new(transport));

        Ok(session)
    }
}

impl DirectConnection {
    pub(crate) fn new(conn: quinn::Connection) -> Result<Self> {
        let peer_cert =
            match conn.peer_identity() {
                None => None,
                Some(ident) => Some(ident.downcast::<Vec<CertificateDer>>().map_err(
                    |uncasted| {
                        anyhow!(
                            "Failed to downcast peer_identity of actual type {}",
                            std::any::type_name_of_val(&*uncasted)
                        )
                    },
                )?),
            };

        let peer = match peer_cert {
            None => Peer::Anonymous,
            Some(der_list) => {
                let der = der_list
                    .first()
                    .ok_or_else(|| anyhow!("expected certificate in some, but empty list found"))?;
                let cert = Certificate::from_der(der.to_vec())?;
                Peer::Certificate(cert)
            }
        };

        Ok(DirectConnection { conn, peer })
    }

    pub fn close(&self, error_code: VarInt, reason: &[u8]) {
        self.conn.close(error_code, reason)
    }
}

impl PartialEq for DirectConnection {
    fn eq(&self, other: &Self) -> bool {
        self.conn.stable_id() == other.conn.stable_id()
    }
}
