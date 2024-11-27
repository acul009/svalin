use std::fmt::Debug;
use std::time::Duration;
use std::collections::BTreeMap;

use anyhow::Result;

pub mod device;
mod first_connect;
pub mod tunnel_manager;

pub mod add_agent;
mod profile;

use device::Device;
pub use first_connect::*;
use svalin_pki::{Certificate, PermCredentials};
use svalin_rpc::commands::ping::Ping;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::rpc::connection::Connection;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tunnel_manager::TunnelManager;

/// flutter_rust_bridge:opaque
pub struct Client {
    rpc: RpcClient,
    _upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: Certificate,
    credentials: PermCredentials,
    device_list: watch::Sender<BTreeMap<Certificate, Device>>,
    tunnel_manager: TunnelManager,
    // TODO: These should not be required here, but should be created and canceled as needed
    background_tasks: JoinSet<()>,
}

impl Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish()
    }
}

impl Client {
    pub async fn device(&self, certificate: Certificate) -> Option<Device> {
        self.device_list.borrow().get(&certificate).cloned()
    }

    pub async fn ping_upstream(&self) -> Result<Duration> {
        self.rpc.upstream_connection().dispatch(Ping).await
    }

    pub fn device_list(&self) -> watch::Ref<BTreeMap<Certificate, Device>> {
        self.device_list.borrow()
    }

    pub fn watch_device_list(&self) -> watch::Receiver<BTreeMap<Certificate, Device>> {
        self.device_list.subscribe()
    }

    pub fn tunnel_manager(&self) -> &TunnelManager {
        &self.tunnel_manager
    }

    pub fn close(&self) {
        self.rpc.close()
    }
}
