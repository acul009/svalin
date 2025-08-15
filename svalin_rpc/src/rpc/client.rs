use std::{net::ToSocketAddrs, sync::Arc, time::Duration};

use crate::{permissions::PermissionHandler, rustls};
use anyhow::{Ok, Result, anyhow};
use quinn::{
    TransportConfig, VarInt, crypto::rustls::QuicClientConfig, rustls::crypto::CryptoProvider,
};
use svalin_pki::Credential;
use tokio::time::{error::Elapsed, timeout};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::{
    command::handler::HandlerCollection,
    connection::{ServeableConnection, direct_connection::DirectConnection},
};

pub struct RpcClient {
    connection: DirectConnection,
    cancel: CancellationToken,
    tasks: TaskTracker,
}

impl RpcClient {
    pub async fn connect(
        address: &str,
        identity: Option<&Credential>,
        verifier: Arc<dyn rustls::client::danger::ServerCertVerifier>,
        cancel: CancellationToken,
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
                id.keypair().rustls_private_key(),
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
            cancel,
            tasks: TaskTracker::new(),
        })
    }

    pub fn upstream_connection(&self) -> DirectConnection {
        self.connection.clone()
    }

    pub async fn close(&self, timeout_duration: Duration) -> Result<(), Elapsed> {
        self.cancel.cancel();
        self.tasks.close();

        let result = timeout(timeout_duration, self.tasks.wait()).await;

        self.connection
            .close(0u32.into(), b"graceful shutdown, goodbye");

        result
    }

    pub async fn serve<P>(&self, commands: HandlerCollection<P>) -> Result<()>
    where
        P: PermissionHandler,
    {
        // Todo: implement canceling and graceful shutdown
        self.upstream_connection()
            .serve(commands, self.cancel.clone())
            .await
    }
}
