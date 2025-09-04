use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Certificate, Credential, EncryptedCredential, ExactVerififier, KnownCertificateVerifier,
};
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};
use tokio::sync::watch;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{debug, error};

use crate::{
    client::tunnel_manager::TunnelManager, shared::commands::agent_list::UpdateAgentList,
    util::location::Location, verifier::upstream_verifier::UpstreamVerifier,
};

use super::Client;

#[derive(Serialize, Deserialize)]
pub(crate) struct Profile {
    pub(crate) username: String,
    pub(crate) upstream_address: String,
    pub(crate) upstream_certificate: Certificate,
    pub(crate) root_certificate: Certificate,
    pub(crate) user_credential: EncryptedCredential,
    pub(crate) device_credential: EncryptedCredential,
}

impl Profile {
    pub(crate) fn new(
        username: String,
        upstream_address: String,
        upstream_certificate: Certificate,
        root_certificate: Certificate,
        user_credential: EncryptedCredential,
        device_credential: EncryptedCredential,
    ) -> Self {
        Self {
            username,
            upstream_address,
            upstream_certificate,
            root_certificate,
            user_credential,
            device_credential,
        }
    }

    pub fn name(&self) -> String {
        format!("{}@{}", self.username, self.upstream_address)
    }
}

impl Client {
    async fn data_dir() -> Result<Location> {
        Location::user_data_dir()?
            .push("client")
            .ensure_exists()
            .await
    }

    async fn profile_dir(profile_name: &str) -> Result<Location> {
        Ok(Self::data_dir().await?.push(profile_name.replace(":", "+")))
    }

    pub async fn list_profiles() -> Result<Vec<String>> {
        let location = Self::data_dir().await?;

        let mut folders = tokio::fs::read_dir(&location).await?;

        let mut profiles = Vec::new();

        while let Some(entry) = folders.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                profiles.push(entry.file_name().to_string_lossy().into_owned());
            }
        }

        Ok(profiles)
    }

    pub async fn add_profile(
        username: String,
        upstream_address: String,
        upstream_certificate: Certificate,
        root_certificate: Certificate,
        user_credentials: Credential,
        device_credentials: Credential,
        password: Vec<u8>,
    ) -> Result<String> {
        let encrypted_user_credential = user_credentials.export(password.clone()).await?;
        let encrypted_device_credential = device_credentials.export(password.clone()).await?;

        let profile = Profile::new(
            username,
            upstream_address,
            upstream_certificate,
            root_certificate,
            encrypted_user_credential,
            encrypted_device_credential,
        );

        let profile_name = profile.name();

        if Self::list_profiles().await?.contains(&profile_name) {
            return Err(anyhow!("profile already exists"));
        }

        Self::save_profile(&profile).await?;

        Ok(profile_name)
    }

    async fn save_profile(profile: &Profile) -> Result<()> {
        let location = Self::profile_dir(&profile.name())
            .await?
            .ensure_exists()
            .await?
            .push("profile.json");

        let json = serde_json::to_string_pretty(profile)?;
        tokio::fs::write(location, json).await?;

        Ok(())
    }

    async fn get_profile(profile_name: &str) -> Result<Option<Profile>> {
        let location = Self::profile_dir(profile_name).await?.push("profile.json");

        if tokio::fs::try_exists(&location).await? {
            let json = tokio::fs::read_to_string(location).await?;
            let profile = serde_json::from_str(&json)?;

            Ok(Some(profile))
        } else {
            Ok(None)
        }
    }

    pub async fn remove_profile(profile_name: &str) -> Result<()> {
        let location = Self::profile_dir(profile_name).await?;

        tokio::fs::remove_dir_all(location).await?;

        Ok(())
    }

    pub async fn open_profile(profile_key: &str, password: Vec<u8>) -> Result<Arc<Self>> {
        let profile = Self::get_profile(&profile_key).await?;

        debug!("Data from profile ready");

        if let Some(profile) = profile {
            debug!("unlocking profile");
            let identity = profile.user_credential.decrypt(password).await?;

            debug!("creating verifier");
            let verifier = UpstreamVerifier::new(
                profile.root_certificate.clone(),
                profile.upstream_certificate.clone(),
            )
            .to_tls_verifier();

            debug!("connecting to server");
            let rpc = RpcClient::connect(
                &profile.upstream_address,
                Some(&identity),
                verifier,
                CancellationToken::new(),
            )
            .await?;

            debug!("connected to server");

            let tunnel_manager = TunnelManager::new();

            let client = Arc::new(Self {
                rpc,
                _upstream_address: profile.upstream_address,
                upstream_certificate: profile.upstream_certificate,
                root_certificate: profile.root_certificate.clone(),
                credentials: identity.clone(),
                device_list: watch::channel(BTreeMap::new()).0,
                tunnel_manager,
                background_tasks: TaskTracker::new(),
                cancel: CancellationToken::new(),
            });

            let list_clone = client.device_list.clone();
            let sync_connection = client.rpc.upstream_connection();
            let tunnel_manager = client.tunnel_manager.clone();
            let cancel = client.cancel.clone();
            let client2 = client.clone();

            client.background_tasks.spawn(async move {
                debug!("subscribing to upstream agent list");
                if let Err(err) = sync_connection
                    .dispatch(UpdateAgentList {
                        client: client2,
                        base_connection: sync_connection.clone(),
                        credentials: identity,
                        list: list_clone,
                        verifier: ExactVerififier::new(profile.root_certificate),
                        tunnel_manager,
                        cancel,
                    })
                    .await
                {
                    error!("error while keeping agent list in sync: {}", err);
                }
            });

            Ok(client)
        } else {
            Err(anyhow!("Profile is empty"))
        }
    }
}
