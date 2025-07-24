use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::time::Duration;
use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Result, anyhow};
use quinn::EndpointConfig;
use quinn::crypto::rustls::QuicServerConfig;
use quinn::rustls::crypto::CryptoProvider;
use svalin_pki::{Certificate, Credential};
use tokio::select;
use tokio::sync::{Mutex, broadcast};
use tokio::time::error::Elapsed;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, error};

use crate::permissions::PermissionHandler;
use crate::rpc::connection::{Connection, ServeableConnection};
use crate::rpc::peer::Peer;
use crate::rustls::{self, server::danger::ClientCertVerifier};

use crate::rpc::command::handler::HandlerCollection;

use super::connection::direct_connection::DirectConnection;
use super::session::Session;

pub mod config_builder;
use config_builder::{RpcCommandBuilder, RpcServerConfigBuilder};

#[derive(Debug)]
pub struct RpcServer {
    config: RpcServerConfig,
    endpoint: quinn::Endpoint,
    connection_data: Mutex<ServerConnectionData>,
    client_status_broadcast: broadcast::Sender<ClientConnectionStatus>,
    tasks: TaskTracker,
}

#[derive(Debug)]
struct RpcServerConfig {
    credentials: Credential,
    client_cert_verifier: Arc<dyn ClientCertVerifier>,
    cancellation_token: CancellationToken,
}

#[derive(Debug, Clone)]
pub struct ClientConnectionStatus {
    pub client: Certificate,
    pub online: bool,
}

#[derive(Debug)]
struct ServerConnectionData {
    latest_connections: BTreeMap<Certificate, DirectConnection>,
}

#[derive(Debug, Clone)]
pub struct Socket(Arc<dyn quinn::AsyncUdpSocket>);

impl RpcServer {
    pub fn build() -> RpcServerConfigBuilder<(), (), (), (), ()> {
        RpcServerConfigBuilder::new()
    }

    pub fn create_socket(addr: SocketAddr) -> std::io::Result<Socket> {
        let std_socket = std::net::UdpSocket::bind(addr)?;
        let runtime = quinn::default_runtime().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "no async runtime found")
        })?;

        let socket = runtime.wrap_udp_socket(std_socket)?;

        Ok(Socket(socket))
    }

    async fn run(
        socket: Socket,
        config: RpcServerConfig,
        command_builder: impl RpcCommandBuilder,
        master_tracker: TaskTracker,
    ) -> Result<Arc<Self>> {
        let server = Self::create(socket, config, master_tracker)?;

        let serve_future = server.clone().serve(command_builder.build(&server).await?);

        server.tasks.spawn(serve_future);

        let cancel_token = server.config.cancellation_token.clone();
        let tasks = server.tasks.clone();
        let endpoint = server.endpoint.clone();

        tokio::spawn(async move {
            // fallback in case the close method is not called
            cancel_token.cancelled().await;
            tasks.close();
            tasks.wait().await;

            endpoint.close(0u32.into(), b"graceful shutdown, goodbye");
        });

        Ok(server)
    }

    fn create(
        socket: Socket,
        config: RpcServerConfig,
        master_tracker: TaskTracker,
    ) -> Result<Arc<Self>> {
        if CryptoProvider::get_default().is_none() {
            let _ = quinn::rustls::crypto::ring::default_provider().install_default();
        }

        let endpoint =
            RpcServer::create_endpoint(socket, &config).context("failed to create rpc endpoint")?;

        let (br_send, _) = broadcast::channel::<ClientConnectionStatus>(10);

        let tasks = TaskTracker::new();
        let tasks2 = tasks.clone();

        master_tracker.spawn(async move {
            tasks2.wait().await;
        });

        Ok(Arc::new(Self {
            endpoint,
            connection_data: Mutex::new(ServerConnectionData {
                latest_connections: BTreeMap::new(),
            }),
            client_status_broadcast: br_send,
            config,
            tasks,
        }))
    }

    fn create_endpoint(socket: Socket, config: &RpcServerConfig) -> Result<quinn::Endpoint> {
        let priv_key = rustls::pki_types::PrivateKeyDer::try_from(
            config.credentials.get_der_key_bytes().to_owned(),
        )
        .map_err(|err| anyhow!(err))?;

        let cert_chain = vec![rustls::pki_types::CertificateDer::from(
            config.credentials.get_certificate().to_der().to_owned(),
        )];

        let crypto = rustls::ServerConfig::builder()
            .with_client_cert_verifier(config.client_cert_verifier.clone())
            .with_single_cert(cert_chain, priv_key)?;

        let config = quinn::ServerConfig::with_crypto(Arc::new(
            QuicServerConfig::try_from(crypto).map_err(|err| anyhow!(err))?,
        ));

        let runtime = quinn::default_runtime().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "no async runtime found")
        })?;

        let endpoint = quinn::Endpoint::new_with_abstract_socket(
            EndpointConfig::default(),
            Some(config),
            socket.0,
            runtime,
        )?;

        Ok(endpoint)
    }

    async fn serve(self: Arc<Self>, commands: HandlerCollection<impl PermissionHandler>) {
        debug!("starting server");

        loop {
            debug!("Waiting for next connection");
            select! {
                _ = self.config.cancellation_token.cancelled() => {
                    debug!("canceling RPC main serve loop");
                    break;
                }
                conn_option = self.endpoint.accept() => {
                    match conn_option {
                        None => {
                            debug!("no more connections");
                        },
                        Some(conn) => {
                            debug!("connection incoming");
                            let serve_connection_future = self.clone().serve_connection(conn, commands.clone());
                            self.tasks.spawn(serve_connection_future);
                        }
                    }
                }
            }
        }
    }

    async fn serve_connection(
        self: Arc<Self>,
        conn: quinn::Incoming,
        commands: HandlerCollection<impl PermissionHandler>,
    ) {
        let result = self.serve_connection_inner(conn, commands).await;

        if let Err(e) = result {
            // TODO: actually handle error
            error!("{}", e);
        }
    }

    async fn serve_connection_inner(
        self: Arc<Self>,
        conn: quinn::Incoming,
        commands: HandlerCollection<impl PermissionHandler>,
    ) -> Result<()> {
        debug!("spawned new task for incoming connection");
        debug!("waiting for connection to get ready...");

        let conn = conn
            .await
            .context("Error when awaiting connection establishment")?;

        debug!("connection established");

        let conn = DirectConnection::new(conn)?;

        // certificate has already been verified by quinn using a custom verifier

        if let Peer::Certificate(cert) = conn.peer() {
            debug!("noting down connection for peer");
            let mut lock = self.connection_data.lock().await;
            lock.latest_connections.insert(cert.clone(), conn.clone());
            let _ = self.client_status_broadcast.send(ClientConnectionStatus {
                client: cert.clone(),
                online: true,
            });

            let on_close_future = self.clone().update_connection_data_on_close(conn.clone());

            self.tasks.spawn(on_close_future);
        }

        conn.serve(
            commands.clone(),
            self.config.cancellation_token.child_token(),
        )
        .await?;

        debug!("connection handled");

        Ok(())
    }

    async fn update_connection_data_on_close(self: Arc<Self>, conn: DirectConnection) {
        debug!("spawned task to update connection data on close");
        conn.closed().await;
        if let Peer::Certificate(cert) = conn.peer() {
            debug!("removing connection data for peer after close");
            let mut lock = self.connection_data.lock().await;
            if let Some(latest_peer_conn) = lock.latest_connections.get(cert) {
                if latest_peer_conn.eq(&conn) {
                    lock.latest_connections.remove(cert);
                    let _ = self.client_status_broadcast.send(ClientConnectionStatus {
                        client: cert.clone(),
                        online: false,
                    });
                }
            }
        }
    }

    pub async fn close(&self, timeout_duration: Duration) -> Result<(), Elapsed> {
        self.config.cancellation_token.cancel();
        self.tasks.close();

        let result = timeout(timeout_duration, self.tasks.wait()).await;

        self.endpoint
            .close(0u32.into(), b"graceful shutdown, goodbye");

        result
    }

    pub async fn open_session_with(&self, peer: Certificate) -> Result<Session> {
        let lock = self.connection_data.lock().await;

        let conn = lock
            .latest_connections
            .get(&peer)
            .ok_or_else(|| anyhow!("no connection to requested peer"))?;

        let (read, write) = conn.open_raw_session().await?;

        let session = Session::new(read, write, conn.peer().clone());

        Ok(session)
    }

    pub fn subscribe_to_connection_status(&self) -> broadcast::Receiver<ClientConnectionStatus> {
        self.client_status_broadcast.subscribe()
    }

    pub async fn get_current_connected_clients(&self) -> BTreeSet<Certificate> {
        self.connection_data
            .lock()
            .await
            .latest_connections
            .keys()
            .cloned()
            .collect()
    }

    pub async fn is_client_connected(&self, client: &Certificate) -> bool {
        self.connection_data
            .lock()
            .await
            .latest_connections
            .contains_key(client)
    }
}
