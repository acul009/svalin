use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    num::NonZeroU16,
    ops::Range,
    sync::Mutex,
};

use svalin_pki::SpkiHash;
use svalin_rpc::rpc::connection::Connection;
use tokio::{net::TcpListener, sync::oneshot};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    client::{
        Client,
        state::{self, tunneling},
        tunnel_manager::tcp::TcpTunnelDispatcher,
    },
    message_streaming::client::SendUpdateError,
};

pub mod tcp;

pub(crate) struct TunnelManager {
    active: Mutex<HashMap<Uuid, CancellationToken>>,
}

#[derive(Debug, thiserror::Error)]
pub enum TunnelCreateError {
    #[error("error connecting to target device: {0}")]
    ConnectionError(#[from] anyhow::Error),
    #[error("error binding local TCP listener: {0}")]
    BindListenerError(#[from] std::io::Error),
    #[error("error updating tunnel state: {0}")]
    StateUpdateError(#[from] SendUpdateError),
    #[error("tunnel could not be opened")]
    Aborted,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn open(
        &self,
        client: &Client,
        tunnel: TunnelDefinition,
    ) -> Result<(), TunnelCreateError> {
        let connection = client.device(tunnel.target.clone()).connection().await?;
        let (send_ready, recv_ready) = oneshot::channel::<()>();
        let id = Uuid::new_v4();

        match &tunnel.config {
            TunnelConfig::Tcp {
                local_port,
                remote_host,
            } => {
                let local_port = local_port.unwrap_or_else(|| {
                    let mut hasher = Fnv1aHasher::default();
                    tunnel.target.hash(&mut hasher);
                    tunnel.name.hash(&mut hasher);
                    stable_port(&hasher)
                });

                let listener = TcpListener::bind(format!("127.0.0.1:{}", local_port)).await?;
                let cancel = client.cancel.child_token();

                {
                    let mut active = self.active.lock().unwrap();
                    active.insert(id.clone(), cancel.clone());
                }

                let state_handle = client.state_handle.clone();
                let target = tunnel.target.clone();
                let remote_host = remote_host.clone();
                state_handle
                    .update(state::Update::Tunnel(tunneling::Update::Opened(
                        tunnel.target.clone(),
                        id.clone(),
                        tunnel.clone(),
                    )))
                    .await?;

                client.background_tasks.spawn(async move {
                    if let Err(err) = connection
                        .dispatch(TcpTunnelDispatcher {
                            listener,
                            cancel,
                            ready: send_ready,
                            remote_host,
                        })
                        .await
                    {
                        tracing::error!("error in tcp tunnel: {err:#}");
                    }
                    let _ = state_handle
                        .update(state::Update::Tunnel(tunneling::Update::Closed(target, id)))
                        .await;

                    //Todo: cleanup cancellation token
                });
            }
        }

        if let Err(_err) = recv_ready.await {
            self.active.lock().unwrap().remove(&id);
            return Err(TunnelCreateError::Aborted);
        }

        Ok(())
    }

    pub fn close(&self, id: &Uuid) {
        if let Some(cancel) = self.active.lock().unwrap().remove(id) {
            cancel.cancel();
        }
    }
}

#[derive(Clone, Debug)]
pub struct TunnelDefinition {
    pub target: SpkiHash,
    pub name: String,
    pub config: TunnelConfig,
}

#[derive(Clone, Debug)]
pub enum TunnelConfig {
    Tcp {
        local_port: Option<NonZeroU16>,
        remote_host: String,
    },
}

const FORWARD_PORT_RANGE: Range<u16> = 10240..65535;

fn stable_port(hasher: &impl Hasher) -> NonZeroU16 {
    let width = u64::from(FORWARD_PORT_RANGE.end - FORWARD_PORT_RANGE.start);
    let stable = FORWARD_PORT_RANGE.start + (hasher.finish() % width) as u16;
    NonZeroU16::new(stable).expect("stable port is always non-zero thanks to FORWARD_PORT_RANGE")
}

#[derive(Clone)]
struct Fnv1aHasher {
    state: u64,
}

impl Default for Fnv1aHasher {
    fn default() -> Self {
        Self {
            state: 0xcbf29ce484222325,
        }
    }
}

impl Hasher for Fnv1aHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= u64::from(byte);
            self.state = self.state.wrapping_mul(0x100000001b3);
        }
    }
}
