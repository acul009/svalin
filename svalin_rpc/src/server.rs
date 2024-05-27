use std::{net::SocketAddr, sync::Arc};

use anyhow::{anyhow, Context, Result};
use quinn::{
    crypto::{self, rustls::QuicServerConfig},
    rustls::{
        pki_types::{CertificateDer, PrivateKeyDer},
        server::danger::ClientCertVerifier,
    },
};
use svalin_pki::PermCredentials;
use tokio::task::JoinSet;
use tracing::debug;

use crate::{connection::DirectConnection, HandlerCollection, Connection};

pub struct Server {
    endpoint: quinn::Endpoint,
    open_connections: JoinSet<()>,
}

impl Server {
    pub fn new(
        addr: SocketAddr,
        credentials: &PermCredentials,
        client_cert_verifier: Arc<dyn ClientCertVerifier>,
    ) -> Result<Self> {
        let endpoint = Server::create_endpoint(addr, &credentials, client_cert_verifier)?;

        Ok(Server {
            endpoint,
            open_connections: JoinSet::new(),
        })
    }

    fn create_endpoint(
        addr: SocketAddr,
        credentials: &PermCredentials,
        client_cert_verifier: Arc<dyn ClientCertVerifier>,
    ) -> Result<quinn::Endpoint> {
        let priv_key = quinn::rustls::pki_types::PrivateKeyDer::try_from(
            credentials.get_key_bytes().to_owned(),
        )
        .map_err(|err| anyhow!(err))?;

        let cert_chain = vec![quinn::rustls::pki_types::CertificateDer::from(
            credentials.get_certificate().to_der().to_owned(),
        )];

        let crypto = quinn::rustls::ServerConfig::builder()
            .with_client_cert_verifier(client_cert_verifier)
            .with_single_cert(cert_chain, priv_key)?;

        let config = quinn::ServerConfig::with_crypto(Arc::new(
            QuicServerConfig::try_from(crypto).map_err(|err| anyhow!(err))?,
        ));

        let endpoint = quinn::Endpoint::server(config, addr)?;

        Ok(endpoint)
    }

    pub async fn run(&mut self, commands: Arc<HandlerCollection>) -> Result<()> {
        debug!("starting server");
        while let Some(conn) = self.endpoint.accept().await {
            debug!("connection incoming");
            let fut = Server::handle_connection(conn.accept()?, commands.clone());
            self.open_connections.spawn(async move {
                debug!("spawn successful");
                if let Err(e) = fut.await {
                    // TODO: actually handle error
                    panic!("{}", e);
                }
                debug!("connection handled");
            });
            debug!("Waiting for next connection");
        }
        todo!()
    }

    async fn handle_connection(
        conn: quinn::Connecting,
        commands: Arc<HandlerCollection>,
    ) -> Result<()> {
        debug!("waiting for connection to get ready...");

        let conn = conn
            .await
            .context("Error when awaiting connection establishment")?;

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

        if let Some(cert) = peer_cert {
            debug!("client cert:\n{:?}", cert.as_ref());
        } else {
            debug!("client did not provide cert")
        }

        debug!("connection established");

        let conn = DirectConnection::new(conn);

        conn.serve(commands).await?;

        Ok(())
    }

    pub fn close(&self) {
        self.endpoint.close(0u32.into(), b"");
    }
}
