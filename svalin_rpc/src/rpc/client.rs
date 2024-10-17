use std::{net::ToSocketAddrs, sync::Arc, time::Duration};

use crate::{permissions::PermissionHandler, rustls};
use anyhow::{anyhow, Ok, Result};
use quinn::{
    crypto::rustls::QuicClientConfig, rustls::crypto::CryptoProvider, TransportConfig, VarInt,
};
use svalin_pki::PermCredentials;

use super::{
    command::handler::HandlerCollection,
    connection::{direct_connection::DirectConnection, ServeableConnection},
};

pub struct RpcClient {
    connection: DirectConnection,
}

impl RpcClient {
    pub async fn connect(
        address: &str,
        identity: Option<&PermCredentials>,
        verifier: Arc<dyn rustls::client::danger::ServerCertVerifier>,
    ) -> Result<RpcClient> {
        if CryptoProvider::get_default().is_none() {
            let _ = quinn::rustls::crypto::ring::default_provider().install_default();
        }

        let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse()?)?;

        let builder = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier);

        let rustls_conf = match identity {
            Some(id) => builder.with_client_auth_cert(
                vec![rustls::pki_types::CertificateDer::from(
                    id.get_certificate().to_der().to_owned(),
                )],
                rustls::pki_types::PrivateKeyDer::try_from(id.get_key_bytes().to_owned())
                    .map_err(|err| anyhow!(err))?,
            )?,
            None => builder.with_no_client_auth(),
        };

        // TODO: lower keepalive - needs higher server timeout
        let mut transport_config = TransportConfig::default();
        transport_config.max_idle_timeout(Some(VarInt::from_u32(10_000).into()));
        transport_config.keep_alive_interval(Some(Duration::from_secs(5)));

        let mut client_config =
            quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(rustls_conf)?));
        client_config.transport_config(Arc::new(transport_config));

        endpoint.set_default_client_config(client_config);

        let split: Vec<&str> = address.split(":").collect();

        let host = *split
            .get(0)
            .ok_or_else(|| anyhow!("missing host in endpoint"))?;

        let port: u16 = split
            .get(1)
            .ok_or_else(|| anyhow!("missing port in endpoint"))?
            .parse()?;

        let addr = (host, port)
            .to_socket_addrs()?
            .find(|a| a.is_ipv4())
            .ok_or_else(|| anyhow!("Unable to resolve Hostname, no IPv6 yet"))?;

        let connection = endpoint.connect(addr, host)?.await?;

        let direct_connection = DirectConnection::new(connection)?;

        Ok(Self {
            connection: direct_connection,
        })
    }

    pub fn upstream_connection(&self) -> DirectConnection {
        self.connection.clone()
    }

    pub fn close(&self) {
        self.connection.close(0u32.into(), b"");
    }

    pub async fn serve<P, Permission>(
        &self,
        commands: HandlerCollection<P, Permission>,
    ) -> Result<()>
    where
        P: PermissionHandler<Permission>,
        Permission: 'static,
    {
        self.upstream_connection().serve(commands).await
    }
}
