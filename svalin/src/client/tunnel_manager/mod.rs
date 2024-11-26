use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures::FutureExt;
use svalin_rpc::{
    commands::forward::ForwardConnection,
    rpc::connection::{direct_connection::DirectConnection, Connection},
};
use tcp::{TcpTunnelCloseError, TcpTunnelConfig, TcpTunnelCreateError, TcpTunnelRunError};
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

pub struct TunnelManagerInner {
    active_tunnels: HashMap<Uuid, Arc<Tunnel>>,
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

    pub fn open(
        &self,
        connection: TunnelConnection,
        config: TunnelConfig,
    ) -> Result<(), TunnelCreateError> {
        let tunnel = Arc::new(Tunnel::open(connection, config)?);
        let id = tunnel.id;
        let manager = self.clone();
        let tunnel_clone = tunnel.clone();

        let mut inner = self.inner.lock().unwrap();

        inner.active_tunnels.insert(id, tunnel);

        inner.join_set.spawn(async move {
            let (id, result) = tunnel_clone.run().await;
            if let Err(err) = result {
                tracing::error!("{err}");
            }

            let mut inner = manager.inner.lock().unwrap();

            inner.active_tunnels.remove(&id);
        });

        todo!()
    }
}

pub struct Tunnel {
    id: Uuid,
    config: TunnelConfig,
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
    Tcp(
        oneshot::Receiver<Result<(), TcpTunnelCloseError>>,
        oneshot::Receiver<TcpTunnelRunError>,
    ),
}

impl Tunnel {
    pub async fn open(
        connection: impl Connection + 'static,
        mut active_recv: watch::Receiver<bool>,
    ) -> (Uuid, TunnelRunResult) {
        (
            self.id,
            self.run_result(connection, running, active_recv).await,
        )
    }

    async fn run_result(
        &self,
        connection: impl Connection + 'static,
        running: oneshot::Sender<Result<(), TunnelCreateError>>,
        mut active_recv: watch::Receiver<bool>,
    ) -> TunnelRunResult {
        match &self.config {
            TunnelConfig::Tcp(config) => {
                let (create, run) = config.run(connection, active_recv);

                create.await
                TunnelRunResult::Tcp(create, run)
            }
        }
    }
}
