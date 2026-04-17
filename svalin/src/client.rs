use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};

// pub mod device;
mod first_connect;
pub mod tunnel_manager;

pub mod add_agent;
mod profile;
pub mod state;

pub use first_connect::*;
use svalin_pki::mls::client::MlsClient;
use svalin_pki::{Certificate, Credential, RootCertificate, SpkiHash};
use svalin_rpc::commands::ping::Ping;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::rpc::connection::Connection;
use tokio::sync::{broadcast, watch};
use tokio::time::error::Elapsed;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tunnel_manager::TunnelManager;

use crate::client::state::{ClientState, ClientStateUpdate};
use crate::message_streaming::client::{ClientMessageDispatcherHandle, ClientStateHandle};
use crate::remote_key_retriever::RemoteKeyRetriever;
use crate::verifier::remote_verifier::RemoteVerifier;

pub struct Client {
    rpc: RpcClient,
    _upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: RootCertificate,
    user_credential: Credential,
    mls: Arc<MlsClient<RemoteKeyRetriever, RemoteVerifier>>,
    tunnel_manager: TunnelManager,
    message_sender: ClientMessageDispatcherHandle,
    state_handle: ClientStateHandle,
    background_tasks: TaskTracker,
    cancel: CancellationToken,
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

    pub async fn subscribe_state(
        &self,
    ) -> Result<(ClientState, broadcast::Receiver<ClientStateUpdate>), anyhow::Error> {
        self.state_handle.subscribe().await
    }

    pub async fn ping_upstream(&self) -> anyhow::Result<Duration> {
        self.rpc
            .upstream_connection()
            .dispatch(Ping)
            .await
            .map_err(|err| anyhow!(err))
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
}

#[derive(Clone, Debug)]
pub enum RemoteData<T> {
    Unavailable,
    Pending,
    Ready(T),
}
