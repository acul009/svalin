use std::{net::SocketAddr, sync::Arc};

use anyhow::{anyhow, Result};
use quinn::{
    crypto::rustls::QuicServerConfig,
    rustls::{
        pki_types::{CertificateDer, PrivateKeyDer},
        server::danger::ClientCertVerifier,
    },
};
use svalin_pki::PermCredentials;
use tokio::task::JoinSet;

use crate::{connection::DirectConnection, Connection, HandlerCollection};

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
        let priv_key =
            quinn::rustls::pki_types::PrivateKeyDer::try_from(credentials.get_key_bytes())
                .or_else(|err| Err(anyhow!(err)))?;

        let cert_chain = vec![quinn::rustls::pki_types::CertificateDer::from(
            credentials.get_certificate().to_der(),
        )];

        let config = quinn::ServerConfig::with_crypto(Server::create_crypto(
            cert_chain,
            priv_key,
            client_cert_verifier,
        )?);

        let endpoint = quinn::Endpoint::server(config, addr)?;

        Ok(endpoint)
    }

    fn create_crypto<'a>(
        cert_chain: Vec<quinn::rustls::pki_types::CertificateDer<'a>>,
        priv_key: PrivateKeyDer,
        client_cert_verifier: Arc<dyn ClientCertVerifier>,
    ) -> Result<Arc<QuicServerConfig>> {
        let mut cfg = quinn::rustls::ServerConfig::builder()
            .with_client_cert_verifier(client_cert_verifier)
            .with_single_cert(cert_chain, priv_key)?;
        Ok(Arc::new(QuicServerConfig::try_from(cfg)?))
    }

    pub async fn run(&mut self, commands: Arc<HandlerCollection>) -> Result<()> {
        println!("starting server");
        while let Some(conn) = self.endpoint.accept().await {
            println!("connection incoming");
            let fut = Server::handle_connection(conn.accept()?, commands.clone());
            self.open_connections.spawn(async move {
                println!("spawn successful");
                if let Err(e) = fut.await {
                    print!("Error: {}", e);
                }
                println!("connection handled");
            });
            println!("Waiting for next connection");
        }
        todo!()
    }

    async fn handle_connection(
        conn: quinn::Connecting,
        commands: Arc<HandlerCollection>,
    ) -> Result<()> {
        println!("waiting for connection to get ready...");

        let conn = conn.await?;

        let peer_cert = match conn.peer_identity() {
            None => Ok(None),
            Some(ident) => match ident.downcast::<CertificateDer>() {
                core::result::Result::Ok(cert) => Ok(Some(cert)),
                Err(_) => Err(anyhow!("Failed to get legitimate identity")),
            },
        }?;

        if let Some(cert) = peer_cert {
            println!("client cert:\n{:?}", cert.as_ref());
        } else {
            println!("client did not provide cert")
        }

        println!("connection established");

        let conn = DirectConnection::new(conn);

        conn.serve(commands).await?;

        Ok(())
    }
}
