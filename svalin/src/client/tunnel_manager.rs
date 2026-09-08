use std::{
    collections::HashMap,
    hash::Hasher,
    num::NonZeroU16,
    ops::Range,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, SpkiHash};
use svalin_rpc::{
    commands::forward::ForwardConnection,
    rpc::{
        connection::{Connection, direct_connection::DirectConnection},
        peer::Peer,
    },
};
use tcp::{TcpTunnelConfig, TcpTunnelCreateError, TcpTunnelRunError};
use thiserror::Error;
use tokio::{
    net::TcpListener,
    sync::{oneshot, watch},
    task::JoinSet,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use uuid::Uuid;

use crate::client::Client;

pub mod tcp;

pub(crate) struct TunnelManager {
    active: HashMap<SpkiHash, HashMap<Uuid, CancellationToken>>,
}

type TunnelConnection = ForwardConnection<DirectConnection>;

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
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

                todo!();
            }
        }

        let mut tunnel = Tunnel::open(connection, config).await?;

        let id = tunnel.id();
        let tunnel_result = tunnel.take_result().unwrap();

        self.active
            .send_modify(|tunnels| match tunnels.get_mut(&certificate) {
                Some(peer_tunnels) => {
                    peer_tunnels.insert(id, tunnel);
                }
                None => {
                    let mut peer_tunnels = HashMap::new();
                    peer_tunnels.insert(id, tunnel);
                    tunnels.insert(certificate.clone(), peer_tunnels);
                }
            });

        let active_tunnels = self.active.clone();

        self.join_set.lock().unwrap().spawn(async move {
            let result = tunnel_result.await_result().await;
            // tracing::trace!("tunnel result: {result:?}");
            if let Err(err) = result {
                tracing::error!("{err}");
            }

            active_tunnels.send_modify(|tunnels| match tunnels.get_mut(&certificate) {
                None => return,
                Some(peer_tunnels) => {
                    peer_tunnels.remove(&id);
                    if peer_tunnels.is_empty() {
                        tunnels.remove(&certificate);
                    }
                }
            });
        });

        Ok(())
    }

    pub fn tunnels(&self) -> watch::Ref<'_, HashMap<Certificate, HashMap<Uuid, Tunnel>>> {
        self.active.borrow()
    }

    pub fn close_tunnel(&self, id: &Uuid) {
        self.active.send_modify(|tunnels| {
            for (_, peer_tunnels) in tunnels.iter_mut() {
                if let Some(tunnel) = peer_tunnels.get_mut(id) {
                    tunnel.close();
                    return;
                }
            }
        });
    }

    pub fn watch_tunnels(&self) -> watch::Receiver<HashMap<Certificate, HashMap<Uuid, Tunnel>>> {
        self.active.subscribe()
    }
}

pub struct TunnelDefinition {
    target: SpkiHash,
    name: String,
    config: TunnelConfig,
}

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
