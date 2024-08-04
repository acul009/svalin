use std::time::Duration;
use std::{collections::BTreeMap, ops::Deref, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Context, Result};

pub mod device;
mod first_connect;
pub mod verifiers;

pub mod add_agent;
mod profile;

use device::Device;
pub use first_connect::*;
use svalin_pki::{Certificate, PermCredentials};
use svalin_rpc::commands::ping::pingDispatcher;
use svalin_rpc::rpc::client::RpcClient;
use tokio::sync::RwLock;
use tokio::task::JoinSet;

use crate::shared::commands::agent_list::AgentListItem;

/// flutter_rust_bridge:opaque
pub struct Client {
    rpc: RpcClient,
    upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: Certificate,
    credentials: PermCredentials,
    device_list: Arc<RwLock<BTreeMap<Certificate, AgentListItem>>>,
    // TODO: These should not be required here, but should be created and canceled as needed
    background_tasks: JoinSet<()>,
}

impl Client {
    pub async fn device(&self, certificate: Certificate) -> Result<Device> {
        let connection = self.rpc.forward_connection(certificate.clone())?;

        Ok(Device::new(connection, certificate))
    }

    pub async fn ping_upstream(&self) -> Result<Duration> {
        self.rpc.upstream_connection().ping().await
    }

    pub async fn device_list(&self) -> Vec<AgentListItem> {
        self.device_list.read().await.values().cloned().collect()
    }

    pub fn close(&self) {
        self.rpc.close()
    }
}
