use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use svalin_rpc::{
    commands::forward::ForwardConnection,
    rpc::connection::{direct_connection::DirectConnection, Connection},
};
use tcp::{
    TcpTunnel, TcpTunnelCloseError, TcpTunnelConfig, TcpTunnelCreateError, TcpTunnelRunError,
};
use thiserror::Error;
use tokio::task::JoinSet;
use uuid::Uuid;

pub mod tcp;

#[derive(Clone)]
pub struct TunnelManager {
    inner: Arc<Mutex<TunnelManagerInner>>,
}

pub struct TunnelManagerInner {
    active_tunnels: HashMap<Uuid, Arc<Tunnel<TunnelConnection>>>,
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

pub struct Tunnel<Connection> {
    id: Uuid,
    tunnel: TunnelType<Connection>,
}

pub enum TunnelType<Connection> {
    Tcp(TcpTunnel<Connection>),
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

#[derive(Debug, Error)]
pub enum TunnelCloseError {
    #[error(transparent)]
    Tcp(#[from] TcpTunnelCloseError),
}

impl<C> Tunnel<C>
where
    C: Connection,
{
    pub fn open(connection: C, config: TunnelConfig) -> Result<Self, TunnelCreateError> {
        let id = Uuid::new_v4();

        let tunnel = match config {
            TunnelConfig::Tcp(config) => TunnelType::Tcp(TcpTunnel::open(connection, config)?),
        };

        Ok(Tunnel { id, tunnel })
    }

    pub async fn run(&self) -> (Uuid, Result<(), TunnelRunError>) {
        (self.id, self.run_result().await)
    }

    async fn run_result(&self) -> Result<(), TunnelRunError> {
        match &self.tunnel {
            TunnelType::Tcp(tunnel) => tunnel.run().await?,
        };

        Ok(())
    }

    pub fn close(&self) -> Result<(), TunnelCloseError> {
        match &self.tunnel {
            TunnelType::Tcp(tunnel) => tunnel.close()?,
        };

        Ok(())
    }
}
