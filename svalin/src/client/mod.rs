use std::fmt::Debug;
use std::sync::RwLock;
use std::time::Duration;
use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;

pub mod device;
mod first_connect;

pub mod add_agent;
mod profile;

use device::Device;
pub use first_connect::*;
use svalin_pki::{Certificate, PermCredentials};
use svalin_rpc::commands::ping::Ping;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::rpc::connection::Connection;
use tokio::task::JoinSet;

/// flutter_rust_bridge:opaque
pub struct Client {
    rpc: RpcClient,
    _upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: Certificate,
    credentials: PermCredentials,
    device_list: Arc<RwLock<BTreeMap<Certificate, Device>>>,
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
        self.device_list.read().unwrap().get(&certificate).cloned()
    }

    pub async fn ping_upstream(&self) -> Result<Duration> {
        self.rpc.upstream_connection().dispatch(Ping).await
    }

    pub fn device_list(&self) -> Vec<Device> {
        self.device_list.read().unwrap().values().cloned().collect()
    }

    pub fn close(&self) {
        self.rpc.close()
    }
}
