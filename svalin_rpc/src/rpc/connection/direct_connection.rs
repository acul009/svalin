use anyhow::{anyhow, Result};
use async_trait::async_trait;
use quinn::{rustls::pki_types::CertificateDer, VarInt};
use svalin_pki::Certificate;
use tracing::debug;

use crate::{
    rpc::peer::Peer,
    transport::session_transport::{SessionTransportReader, SessionTransportWriter},
};

use super::{ConnectionBase, ServeableConnectionBase};

#[derive(Debug, Clone)]
pub struct DirectConnection {
    conn: quinn::Connection,
    peer: Peer,
}

#[async_trait]
impl ConnectionBase for DirectConnection {
    async fn open_raw_session(
        &self,
    ) -> Result<(
        Box<dyn SessionTransportReader>,
        Box<dyn SessionTransportWriter>,
    )> {
        let transport = self.conn.open_bi().await.map_err(|err| anyhow!(err))?;

        Ok((Box::new(transport.1), Box::new(transport.0)))
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
    async fn accept_raw_session(
        &self,
    ) -> Result<(
        Box<dyn SessionTransportReader>,
        Box<dyn SessionTransportWriter>,
    )> {
        let transport = self.conn.accept_bi().await.map_err(|err| anyhow!(err))?;

        Ok((Box::new(transport.1), Box::new(transport.0)))
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

        debug!("connection with peer: {peer:?}");

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
