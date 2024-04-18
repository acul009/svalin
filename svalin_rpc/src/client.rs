use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use anyhow::{anyhow, Result};
use quinn::crypto;
use svalin_pki::PermCredentials;

pub struct Client;

impl Client {
    pub async fn connect(
        host: &str,
        identity: Option<PermCredentials>,
        verifier: Arc<dyn rustls::client::ServerCertVerifier>,
    ) -> Result<Client> {
        let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse()?)?;

        let builder = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(verifier);

        let rustls_conf = match identity {
            Some(id) => builder.with_client_auth_cert(
                vec![rustls::Certificate(
                    id.get_certificate().to_der().to_owned(),
                )],
                rustls::PrivateKey(id.get_key_bytes().to_owned()),
            )?,
            None => builder.with_no_client_auth(),
        };

        let client_config = quinn::ClientConfig::new(Arc::new(rustls_conf));

        endpoint.set_default_client_config(client_config);

        let addr = host
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("Unable to resolve Hostname"))?;

        let connecting = endpoint.connect(addr, host)?.await?;

        todo!()
    }
}
