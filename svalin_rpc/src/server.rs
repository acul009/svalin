use std::{net::SocketAddr, sync::Arc};

use anyhow::{anyhow, Result};
use rustls::PrivateKey;
use svalin_pki::PermCredentials;
use tokio::task::JoinSet;

use crate::{connection::DirectConnection, Connection, HandlerCollection};

pub struct Server {
    endpoint: quinn::Endpoint,
    open_connections: JoinSet<()>,
}

impl Server {
    pub fn new(addr: SocketAddr, credentials: &PermCredentials) -> Result<Self> {
        let endpoint = Server::create_endpoint(addr, &credentials)?;

        Ok(Server {
            endpoint,
            open_connections: JoinSet::new(),
        })
    }

    fn create_endpoint(addr: SocketAddr, credentials: &PermCredentials) -> Result<quinn::Endpoint> {
        let priv_key = rustls::PrivateKey(credentials.get_key_bytes().to_owned());

        let cert_chain = vec![rustls::Certificate(
            credentials.get_certificate().to_der().to_owned(),
        )];

        let config = quinn::ServerConfig::with_crypto(Server::create_crypto(cert_chain, priv_key)?);

        let endpoint = quinn::Endpoint::server(config, addr)?;

        Ok(endpoint)
    }

    fn create_crypto(
        cert_chain: Vec<rustls::Certificate>,
        priv_key: PrivateKey,
    ) -> Result<Arc<rustls::ServerConfig>> {
        let mut cfg = rustls::ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_no_client_auth()
            .with_single_cert(cert_chain, priv_key)?;
        cfg.max_early_data_size = u32::MAX;
        Ok(Arc::new(cfg))
    }

    pub async fn run(&mut self, commands: Arc<HandlerCollection>) -> Result<()> {
        println!("starting server");
        while let Some(conn) = self.endpoint.accept().await {
            println!("connection incoming");
            let fut = Server::handle_connection(conn, commands.clone());
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
            Some(ident) => match ident.downcast::<rustls::Certificate>() {
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
