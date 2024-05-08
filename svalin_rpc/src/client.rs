use std::{net::ToSocketAddrs, sync::Arc};

use anyhow::{anyhow, Ok, Result};
use quinn::crypto::rustls::QuicClientConfig;
use svalin_pki::PermCredentials;

pub struct Client {
    connection: quinn::Connection,
}

impl Client {
    pub async fn connect(
        address: String,
        identity: Option<&PermCredentials>,
        verifier: Arc<dyn quinn::rustls::client::danger::ServerCertVerifier>,
    ) -> Result<Client> {
        let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse()?)?;

        let builder = quinn::rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier);

        let rustls_conf = match identity {
            Some(id) => builder.with_client_auth_cert(
                vec![quinn::rustls::pki_types::CertificateDer::from(
                    id.get_certificate().to_der().to_owned(),
                )],
                quinn::rustls::pki_types::PrivateKeyDer::try_from(id.get_key_bytes().to_owned())
                    .map_err(|err| anyhow!(err))?,
            )?,
            None => builder.with_no_client_auth(),
        };

        let client_config =
            quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(rustls_conf)?));

        endpoint.set_default_client_config(client_config);

        let url = url::Url::parse(&format!("svalin://{address}"))?;

        let host = url
            .host_str()
            .ok_or_else(|| anyhow!("missing host in url"))?;

        // default port
        let port = url.port().unwrap_or(1234);

        let addr = (host, port)
            .to_socket_addrs()?
            .find(|a| a.is_ipv4())
            .ok_or_else(|| anyhow!("Unable to resolve Hostname, no IPv6 yet"))?;

        let connection = endpoint.connect(addr, host)?.await?;

        println!("{:?}", connection.handshake_data().unwrap());

        Ok(Self { connection })
    }

    pub fn upstream_connection(&self) -> crate::DirectConnection {
        crate::DirectConnection::new(self.connection.clone())
    }

    pub fn close(&self) {
        self.connection.close(0u32.into(), b"");
    }
}
