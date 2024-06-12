use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use quinn::{RecvStream, SendStream};
use svalin_pki::Certificate;
use tokio::task::JoinSet;
use tracing::field::debug;
use tracing::{debug, error};

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
pub trait Connection: Send + Sync {
    async fn serve(&self, commands: Arc<HandlerCollection>) -> Result<()>;

    async fn open_session(&self, command_key: String) -> Result<Session<SessionOpen>>;

    async fn closed(&self);
}

pub struct DirectConnection {
    conn: quinn::Connection,
    peer: Peer,
}

#[async_trait]
impl crate::rpc::connection::Connection for DirectConnection {
    async fn serve(&self, commands: Arc<HandlerCollection>) -> Result<()> {
        debug!("waiting for incoming data stream");
        let mut open_sessions = JoinSet::<()>::new();

        loop {
            match self.accept_session().await {
                Ok(session) => {
                    let commands2 = commands.clone();
                    open_sessions.spawn(async move {
                        let res = session.handle(commands2).await;
                        if let Err(e) = res {
                            // TODO: Actually handle Error
                            error!("{}", e);
                        }
                    });
                }
                Err(_err) => while open_sessions.join_next().await.is_some() {},
            }
        }
    }

    async fn open_session(&self, command_key: String) -> Result<Session<SessionOpen>> {
        debug!("creating transport");

        let transport: CombinedTransport<SendStream, RecvStream> = self
            .conn
            .open_bi()
            .await
            .map_err(|err| anyhow!(err))?
            .into();

        debug!("transport created, pass to session");

        let session = Session::new(Box::new(transport));

        debug!("requesting session");

        let session = session.request_session(command_key).await?;

        debug!("session request successful");

        Ok(session)
    }

    async fn closed(&self) {
        self.closed().await
    }
}

impl DirectConnection {
    pub(crate) fn new(conn: quinn::Connection) -> Self {
        let der = conn.peer_identity();

        let peer = match der {
            Some(der_any) => {
                let downcast_result: Result<Box<Vec<crate::rustls::pki_types::CertificateDer>>, _> =
                    der_any.downcast();

                match downcast_result {
                    //TODO
                    Ok(der) => Peer::Anonymous,
                    Err(_) => Peer::Anonymous,
                }
            }
            None => Peer::Anonymous,
        };

        DirectConnection { conn, peer }
    }

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
