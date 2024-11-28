use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use svalin_rpc::{
    commands::forward::ForwardConnection,
    rpc::{
        connection::{direct_connection::DirectConnection, Connection},
        peer::Peer,
    },
};
use tcp::{TcpTunnelConfig, TcpTunnelCreateError, TcpTunnelRunError};
use thiserror::Error;
use tokio::{
    sync::{oneshot, watch},
    task::JoinSet,
};
use uuid::Uuid;

pub mod tcp;

#[derive(Clone)]
pub struct TunnelManager {
    active_tunnels: watch::Sender<HashMap<[u8; 32], HashMap<Uuid, Tunnel>>>,
    join_set: Arc<Mutex<JoinSet<()>>>,
}

struct TunnelManagerInner {
    active_tunnels: HashMap<Uuid, Tunnel>,
    join_set: JoinSet<()>,
}

type TunnelConnection = ForwardConnection<DirectConnection>;

impl TunnelManager {
    pub fn new() -> Self {
        let (active_tunnels, _) = watch::channel(HashMap::new());
        Self {
            active_tunnels,
            join_set: Arc::new(Mutex::new(JoinSet::new())),
        }
    }

    pub async fn open(
        &self,
        connection: TunnelConnection,
        config: TunnelConfig,
    ) -> Result<(), TunnelCreateError> {
        let fingerprint = match connection.peer() {
            Peer::Anonymous => return Err(TunnelCreateError::NoPeerOnConnection),
            Peer::Certificate(certificate) => certificate.fingerprint(),
        };

        let mut tunnel = Tunnel::open(connection, config).await?;

        let id = tunnel.id();
        let tunnel_result = tunnel.take_result().unwrap();

        self.active_tunnels
            .send_modify(|tunnels| match tunnels.get_mut(&fingerprint) {
                Some(peer_tunnels) => {
                    peer_tunnels.insert(id, tunnel);
                }
                None => {
                    let mut peer_tunnels = HashMap::new();
                    peer_tunnels.insert(id, tunnel);
                    tunnels.insert(fingerprint, peer_tunnels);
                }
            });

        let active_tunnels = self.active_tunnels.clone();

        self.join_set.lock().unwrap().spawn(async move {
            let result = tunnel_result.await_result().await;
            if let Err(err) = result {
                tracing::error!("{err}");
            }

            active_tunnels.send_modify(|tunnels| match tunnels.get_mut(&fingerprint) {
                None => return,
                Some(peer_tunnels) => {
                    peer_tunnels.remove(&id);
                    if peer_tunnels.is_empty() {
                        tunnels.remove(&fingerprint);
                    }
                }
            });
        });

        Ok(())
    }

    pub fn watch_tunnels(&self) -> watch::Receiver<HashMap<[u8; 32], HashMap<Uuid, Tunnel>>> {
        self.active_tunnels.subscribe()
    }
}

pub struct Tunnel {
    id: Uuid,
    config: TunnelConfig,
    run_result: Option<TunnelRunResult>,
    active_send: watch::Sender<bool>,
    peer: Peer,
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.active_send.send(false).unwrap();
    }
}

pub enum TunnelConfig {
    Tcp(TcpTunnelConfig),
}

#[derive(Debug, Error)]
pub enum TunnelCreateError {
    #[error("given connection has no peer")]
    NoPeerOnConnection,
    #[error(transparent)]
    Tcp(#[from] TcpTunnelCreateError),
}

#[derive(Debug, Error)]
pub enum TunnelRunError {
    #[error(transparent)]
    Tcp(#[from] TcpTunnelRunError),
}

pub enum TunnelRunResult {
    Tcp(oneshot::Receiver<TcpTunnelRunError>),
}

impl TunnelRunResult {
    pub async fn await_result(self) -> Result<(), TunnelRunError> {
        match self {
            TunnelRunResult::Tcp(result) => match result.await {
                Ok(err) => Err(err.into()),
                Err(_) => Ok(()),
            },
        }
    }
}

impl Tunnel {
    pub async fn open(
        connection: impl Connection + 'static,
        config: TunnelConfig,
    ) -> Result<Tunnel, TunnelCreateError> {
        let peer = connection.peer().clone();
        let (active_send, active_recv) = watch::channel(false);
        let run_result = Some(match &config {
            TunnelConfig::Tcp(config) => {
                TunnelRunResult::Tcp(config.run(connection, active_recv).await?)
            }
        });
        let id = Uuid::new_v4();

        Ok(Self {
            id,
            config,
            run_result,
            active_send,
            peer,
        })
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn config(&self) -> &TunnelConfig {
        &self.config
    }

    pub fn peer(&self) -> &Peer {
        &self.peer
    }

    pub fn take_result(&mut self) -> Option<TunnelRunResult> {
        self.run_result.take()
    }

    pub fn close(&mut self) {
        // we only care about shutting down the tunnel.
        // if it's already closed, we don't need to do anything
        let _ = self.active_send.send(false);
    }
}
