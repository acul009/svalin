use std::collections::BTreeMap;
use std::fmt::Debug;
use std::time::Duration;

use anyhow::{Result, anyhow};

pub mod device;
mod first_connect;
pub mod tunnel_manager;

pub mod add_agent;
mod profile;

use device::Device;
pub use first_connect::*;
use svalin_pki::{Certificate, Credential, Fingerprint, SpkiHash};
use svalin_rpc::commands::ping::Ping;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::rpc::connection::Connection;
use tokio::sync::watch;
use tokio::time::error::Elapsed;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tunnel_manager::TunnelManager;

use crate::shared::commands::get_user_certs::{GetUserKeyPackages, UserCertificates};

pub struct Client {
    rpc: RpcClient,
    _upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: Certificate,
    user_credential: Credential,
    _device_credential: Credential,
    device_list: watch::Sender<BTreeMap<Certificate, Device>>,
    tunnel_manager: TunnelManager,
    // TODO: These should not be required here, but should be created and canceled as needed
    background_tasks: TaskTracker,
    cancel: CancellationToken,
}

impl Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish()
    }
}

impl Client {
    pub fn device(&self, certificate: &Certificate) -> Option<Device> {
        self.device_list.borrow().get(certificate).cloned()
    }

    pub async fn ping_upstream(&self) -> anyhow::Result<Duration> {
        self.rpc
            .upstream_connection()
            .dispatch(Ping)
            .await
            .map_err(|err| anyhow!(err))
    }

    pub fn device_list<'a>(&'a self) -> watch::Ref<'a, BTreeMap<Certificate, Device>> {
        self.device_list.borrow()
    }

    pub fn watch_device_list(&self) -> watch::Receiver<BTreeMap<Certificate, Device>> {
        self.device_list.subscribe()
    }

    pub fn tunnel_manager(&self) -> &TunnelManager {
        &self.tunnel_manager
    }

    pub async fn get_user_certificates(
        &self,
        spki_hash: &SpkiHash,
    ) -> anyhow::Result<UserCertificates> {
        let certs = self
            .rpc
            .upstream_connection()
            .dispatch(GetUserKeyPackages(spki_hash))
            .await?;

        Ok(certs)
    }

    pub(crate) fn get_key_packages(&self, certs: Vec<Certificate>) {
        todo!()
    }

    pub(crate) fn cancellation_token(&self) -> &CancellationToken {
        &self.cancel
    }

    pub(crate) fn background_tasks(&self) -> &TaskTracker {
        &self.background_tasks
    }

    pub async fn close(&self, timeout_duration: Duration) -> Result<(), Elapsed> {
        self.cancel.cancel();
        self.background_tasks.close();

        let result = timeout(timeout_duration, self.background_tasks.wait()).await;

        let result2 = self.rpc.close(timeout_duration).await;

        match result {
            Err(e) => Err(e),
            Ok(()) => result2,
        }
    }
}
