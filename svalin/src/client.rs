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
use futures::{StreamExt, TryStreamExt};
use svalin_pki::mls::OpenMlsProvider;
use svalin_pki::mls::client::MlsClient;
use svalin_pki::mls::key_package::{KeyPackage, KeyPackageError};
use svalin_pki::{Certificate, Credential, RootCertificate, SpkiHash};
use svalin_rpc::commands::ping::Ping;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::rpc::connection::{Connection, ConnectionDispatchError};
use tokio::sync::watch;
use tokio::time::error::Elapsed;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tunnel_manager::TunnelManager;

use crate::shared::commands::get_key_packages::{GetKeyPackages, GetKeyPackagesDispatcherError};
use crate::verifier::remote_session_verifier::RemoteSessionVerifier;

pub struct Client {
    rpc: RpcClient,
    _upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: RootCertificate,
    user_credential: Credential,
    mls: MlsClient,
    device_list: watch::Sender<BTreeMap<SpkiHash, Device>>,
    tunnel_manager: TunnelManager,
    // TODO: These should not be required here, but should be created and canceled as needed
    background_tasks: TaskTracker,
    cancel: CancellationToken,
    session_verifier: RemoteSessionVerifier,
}

impl Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish()
    }
}

impl Client {
    pub(crate) fn user_credential(&self) -> &Credential {
        &self.user_credential
    }

    pub(crate) fn root_certificate(&self) -> &RootCertificate {
        &self.root_certificate
    }

    pub(crate) fn upstream_certificate(&self) -> &Certificate {
        &self.upstream_certificate
    }

    pub(crate) fn mls(&self) -> &MlsClient {
        &self.mls
    }

    pub fn device(&self, spki_hash: &SpkiHash) -> Option<Device> {
        self.device_list.borrow().get(spki_hash).cloned()
    }

    pub async fn ping_upstream(&self) -> anyhow::Result<Duration> {
        self.rpc
            .upstream_connection()
            .dispatch(Ping)
            .await
            .map_err(|err| anyhow!(err))
    }

    pub fn device_list<'a>(&'a self) -> watch::Ref<'a, BTreeMap<SpkiHash, Device>> {
        self.device_list.borrow()
    }

    pub fn watch_device_list(&self) -> watch::Receiver<BTreeMap<SpkiHash, Device>> {
        self.device_list.subscribe()
    }

    pub fn tunnel_manager(&self) -> &TunnelManager {
        &self.tunnel_manager
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

    pub(crate) async fn get_key_packages(
        &self,
        entities: &[Certificate],
    ) -> Result<Vec<KeyPackage>, GetKeyPackagesError> {
        let key_packages = self
            .rpc
            .upstream_connection()
            .dispatch(GetKeyPackages::new(entities))
            .await?;

        let key_packages = futures::stream::iter(key_packages.into_iter())
            .map(|key_package| {
                key_package.verify(
                    self.mls().provider().crypto(),
                    self.mls().protocol_version(),
                    &self.session_verifier,
                )
            })
            .buffer_unordered(10)
            .try_collect::<Vec<_>>()
            .await?;

        Ok(key_packages)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetKeyPackagesError {
    #[error("error in dispatcher: {0}")]
    Dispatcher(#[from] ConnectionDispatchError<GetKeyPackagesDispatcherError>),
    #[error("error verifying key package: {0}")]
    VerifyError(#[from] KeyPackageError),
}
