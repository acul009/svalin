use std::collections::{BTreeMap, BTreeSet};
use std::{net::SocketAddr, sync::Arc};

use anyhow::{anyhow, Context, Result};
use quinn::crypto::rustls::QuicServerConfig;
use quinn::rustls::crypto::CryptoProvider;
use svalin_pki::{Certificate, PermCredentials};
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinSet;
use tracing::debug;

use crate::permissions::PermissionHandler;
use crate::rpc::connection::{ConnectionBase, ServeableConnection};
use crate::rpc::peer::Peer;
use crate::rustls::{self, server::danger::ClientCertVerifier};

use crate::rpc::command::handler::HandlerCollection;
use crate::transport::session_transport::{SessionTransportReader, SessionTransportWriter};

use super::connection::direct_connection::DirectConnection;
use super::session::Session;

#[derive(Debug, Clone)]
pub struct ClientConnectionStatus {
    pub client: Certificate,
    pub online: bool,
}

#[derive(Debug, Clone)]
pub struct RpcServer {
    endpoint: quinn::Endpoint,
    data: Arc<Mutex<ServerData>>,
    broadcast: broadcast::Sender<ClientConnectionStatus>,
}

#[derive(Debug)]
struct ServerData {
    connection_join_set: JoinSet<()>,
    latest_connections: BTreeMap<Certificate, DirectConnection>,
}

impl RpcServer {
    pub fn new(
        addr: SocketAddr,
        credentials: &PermCredentials,
        client_cert_verifier: Arc<dyn ClientCertVerifier>,
    ) -> Result<Self> {
        if CryptoProvider::get_default().is_none() {
            let _ = quinn::rustls::crypto::ring::default_provider().install_default();
        }

        let endpoint = RpcServer::create_endpoint(addr, credentials, client_cert_verifier)
            .context("failed to create rpc endpoint")?;

        let (br_send, _) = broadcast::channel::<ClientConnectionStatus>(10);

        Ok(RpcServer {
            endpoint,
            data: Arc::new(Mutex::new(ServerData {
                connection_join_set: JoinSet::new(),
                latest_connections: BTreeMap::new(),
            })),
            broadcast: br_send,
        })
    }

    fn create_endpoint(
        addr: SocketAddr,
        credentials: &PermCredentials,
        client_cert_verifier: Arc<dyn ClientCertVerifier>,
    ) -> Result<quinn::Endpoint> {
        let priv_key =
            rustls::pki_types::PrivateKeyDer::try_from(credentials.get_key_bytes().to_owned())
                .map_err(|err| anyhow!(err))?;

        let cert_chain = vec![rustls::pki_types::CertificateDer::from(
            credentials.get_certificate().to_der().to_owned(),
        )];

        let crypto = rustls::ServerConfig::builder()
            .with_client_cert_verifier(client_cert_verifier)
            .with_single_cert(cert_chain, priv_key)?;

        let config = quinn::ServerConfig::with_crypto(Arc::new(
            QuicServerConfig::try_from(crypto).map_err(|err| anyhow!(err))?,
        ));

        let endpoint =
            quinn::Endpoint::server(config, addr).context("failed to create quinn endpoint")?;

        Ok(endpoint)
    }

    pub async fn run<P>(&self, commands: HandlerCollection<P>) -> Result<()>
    where
        P: PermissionHandler,
        P::Permission: 'static,
    {
        debug!("starting server");
        while let Some(conn) = self.endpoint.accept().await {
            debug!("connection incoming");
            let fut = RpcServer::handle_connection(
                conn.accept()?,
                commands.clone(),
                self.data.clone(),
                self.broadcast.clone(),
            );
            let mut lock = self.data.lock().await;
            lock.connection_join_set.spawn(async move {
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

    async fn handle_connection<P>(
        conn: quinn::Connecting,
        commands: HandlerCollection<P>,
        data: Arc<Mutex<ServerData>>,
        broadcast: broadcast::Sender<ClientConnectionStatus>,
    ) -> Result<()>
    where
        P: PermissionHandler,
    {
        debug!("waiting for connection to get ready...");

        let conn = conn
            .await
            .context("Error when awaiting connection establishment")?;

        // TODO: verify cert

        debug!("connection established");

        let conn = DirectConnection::new(conn)?;

        if let Peer::Certificate(cert) = conn.peer() {
            debug!("noting down connection for peer");
            let mut lock = data.lock().await;
            lock.latest_connections.insert(cert.clone(), conn.clone());
            let _ = broadcast.send(ClientConnectionStatus {
                client: cert.clone(),
                online: true,
            });
            let conn2 = conn.clone();
            let data2 = data.clone();
            let cert2 = cert.clone();
            lock.connection_join_set.spawn(async move {
                conn2.closed().await;
                let mut lock = data2.lock().await;
                if let Some(latest_peer_conn) = lock.latest_connections.get(&cert2) {
                    if latest_peer_conn.eq(&conn2) {
                        lock.latest_connections.remove(&cert2);
                        let _ = broadcast.send(ClientConnectionStatus {
                            client: cert2.clone(),
                            online: false,
                        });
                    }
                }
            });
        }

        conn.serve(commands).await?;

        Ok(())
    }

    pub fn close(&self) {
        let data = self.data.clone();
        tokio::spawn(async move {
            data.lock().await.connection_join_set.abort_all();
        });
        self.endpoint.close(0u32.into(), b"");
    }

    pub async fn open_session_with(&self, peer: Certificate) -> Result<Session> {
        let lock = self.data.lock().await;

        let conn = lock
            .latest_connections
            .get(&peer)
            .ok_or_else(|| anyhow!("no connection to requested peer"))?;

        let (read, write) = conn.open_raw_session().await?;

        let session = Session::new(read, write, conn.peer().clone());

        Ok(session)
    }

    pub fn subscribe_to_connection_status(&self) -> broadcast::Receiver<ClientConnectionStatus> {
        self.broadcast.subscribe()
    }

    pub async fn get_current_connected_clients(&self) -> BTreeSet<Certificate> {
        self.data
            .lock()
            .await
            .latest_connections
            .keys()
            .cloned()
            .collect()
    }

    pub async fn is_client_connected(&self, client: &Certificate) -> bool {
        self.data
            .lock()
            .await
            .latest_connections
            .contains_key(client)
    }
}
