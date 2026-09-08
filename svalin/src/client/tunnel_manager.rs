use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    num::NonZeroU16,
    ops::Range,
    sync::Mutex,
};

use svalin_pki::{Certificate, SpkiHash};
use svalin_rpc::{
    commands::forward::ForwardConnection,
    rpc::connection::{Connection, direct_connection::DirectConnection},
};
use tokio::{net::TcpListener, sync::watch};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use uuid::Uuid;

use crate::client::{
    Client,
    state::{self, tunneling},
    tunnel_manager::tcp::TcpTunnelDispatcher,
};

pub mod tcp;

pub(crate) struct TunnelManager {
    active: Mutex<HashMap<Uuid, CancellationToken>>,
}

type TunnelConnection = ForwardConnection<DirectConnection>;

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
        let connection = client.device(tunnel.target).connection().await?;

        match tunnel.config {
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

                let active = self.active.lock().unwrap();
                let id = Uuid::new_v4();
                active.insert(id.clone(), cancel.clone());

                let state_handle = client.state_handle.clone();
                state_handle
                    .update(state::Update::Tunnel(tunneling::Update::Opened(
                        tunnel.target.clone(),
                        id.clone(),
                        tunnel,
                    )))
                    .await?;

                client.background_tasks.spawn(async move {
                    if let Err(err) = connection
                        .dispatch(TcpTunnelDispatcher { listener, cancel })
                        .await
                    {
                        tracing::error!("error in tcp tunnel: {err:#}");
                    }
                    let _ = state_handle
                        .update(state::Update::Tunnel(tunneling::Update::Closed(
                            tunnel.target,
                            id,
                        )))
                        .await;
                });
            }
        }

        Ok(())
    }

    pub fn close_tunnel(&self, id: &Uuid) {
        todo!()
    }

    pub fn watch_tunnels(&self) -> watch::Receiver<HashMap<Certificate, HashMap<Uuid, Tunnel>>> {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub struct TunnelDefinition {
    target: SpkiHash,
    name: String,
    config: TunnelConfig,
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
