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
    inner: Arc<Mutex<TunnelManagerInner>>,
}

struct TunnelManagerInner {
    active_tunnels: HashMap<Uuid, Tunnel>,
    join_set: JoinSet<()>,
}

type TunnelConnection = ForwardConnection<DirectConnection>;

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TunnelManagerInner {
                active_tunnels: HashMap::new(),
                join_set: JoinSet::new(),
            })),
        }
    }

    pub async fn open(
        &self,
        connection: TunnelConnection,
        config: TunnelConfig,
    ) -> Result<(), TunnelCreateError> {
        let mut tunnel = Tunnel::open(connection, config).await?;

        let id = tunnel.id();
        let manager = self.clone();
        let tunnel_result = tunnel.take_result().unwrap();

        let mut inner = self.inner.lock().unwrap();

        inner.active_tunnels.insert(id, tunnel);

        inner.join_set.spawn(async move {
            let result = tunnel_result.await_result().await;
            if let Err(err) = result {
                tracing::error!("{err}");
            }

            let mut inner = manager.inner.lock().unwrap();

            inner.active_tunnels.remove(&id);
        });

        Ok(())
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
