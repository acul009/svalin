use std::{sync::Arc, time::Duration};

use anyhow::{Result, anyhow};
use openmls_sqlx_storage::SqliteStorageProvider;
use serde::{Deserialize, Serialize};
use svalin_client_store::ClientStore;
use svalin_pki::{
    Certificate, Credential, EncryptedCredential, ExactVerififier, KnownCertificateVerifier,
    RootCertificate, UnverifiedCertificate, get_current_timestamp, mls::client::MlsClient,
};
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{debug, error};

use crate::{
    client::tunnel_manager::TunnelManager,
    message_streaming::client::{ClientMessageDispatcher, ClientMessageReceiver},
    remote_key_retriever::RemoteKeyRetriever,
    shared::commands::update_user_mls::UpdateUserMls,
    util::location::{Location, LocationError},
    verifier::remote_verifier::RemoteVerifier,
};

use super::Client;

#[derive(Serialize, Deserialize)]
pub(crate) struct Profile {
    pub(crate) username: String,
    pub(crate) upstream_address: String,
    pub(crate) upstream_certificate: UnverifiedCertificate,
    pub(crate) root_certificate: UnverifiedCertificate,
    pub(crate) user_credential: EncryptedCredential,
    pub(crate) device_credential: EncryptedCredential,
}

impl Profile {
    pub(crate) fn new(
        username: String,
        upstream_address: String,
        upstream_certificate: Certificate,
        root_certificate: RootCertificate,
        user_credential: EncryptedCredential,
        device_credential: EncryptedCredential,
    ) -> Self {
        Self {
            username,
            upstream_address,
            upstream_certificate: upstream_certificate.to_unverified(),
            root_certificate: root_certificate.to_unverified(),
            user_credential,
            device_credential,
        }
    }

    pub fn name(&self) -> String {
        format!("{}@{}", self.username, self.upstream_address)
    }

    pub async fn profile_dir(&self) -> Result<Location> {
        Client::profile_dir(&self.name()).await
    }
}

impl Client {
    async fn data_dir() -> Result<Location, LocationError> {
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
        root_certificate: RootCertificate,
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

        // debug!("Data from profile ready");

        if let Some(profile) = profile {
            let mls_db_path = profile.profile_dir().await?.push("mls-store.sqlite");
            let client_db_path = profile.profile_dir().await?.push("client-store.sqlite");
            // debug!("unlocking profile");
            let user_credential = profile.user_credential.decrypt(password.clone()).await?;
            let device_credential = profile.device_credential.decrypt(password.clone()).await?;
            let root_certificate = profile.root_certificate.use_as_root()?;
            let upstream_certificate = profile
                .upstream_certificate
                .verify_signature(&root_certificate, get_current_timestamp())?;

            // debug!("creating verifier");
            let verifier = ExactVerififier::new(upstream_certificate.clone()).to_tls_verifier();

            // debug!("connecting to server");
            let rpc = RpcClient::connect(
                &profile.upstream_address,
                Some(&device_credential),
                verifier,
                CancellationToken::new(),
            )
            .await?;

            let remote_verifier =
                RemoteVerifier::new(root_certificate.clone(), rpc.upstream_connection());

            // debug!("connected to server");

            // debug!("opening sqlite database: {}", db_path.display());
            let url = mls_db_path
                .as_path()
                .to_str()
                .ok_or_else(|| anyhow!("db_path was not valid UTF-8"))?;
            let storage_provider = SqliteStorageProvider::open(&url).await?;
            let key_retriever =
                RemoteKeyRetriever::new(rpc.upstream_connection(), root_certificate.clone());

            let mls = Arc::new(MlsClient::new(
                device_credential.clone(),
                storage_provider.into(),
                key_retriever.clone(),
                remote_verifier.clone(),
            )?);

            let tunnel_manager = TunnelManager::new();

            let (dispatcher_handle, message_dispatcher) = ClientMessageDispatcher::new();

            // Starting Background Tasks
            let background_tasks = TaskTracker::new();
            let cancel = CancellationToken::new();

            let connection = rpc.upstream_connection();
            background_tasks.spawn(async move {
                if let Err(err) = connection.dispatch(message_dispatcher).await {
                    error!("failed to send messages to server: {:#}", err);
                }
            });

            let client_store = Arc::new(ClientStore::open(client_db_path).await?);

            let (message_receiver, client_state_handle) = ClientMessageReceiver::initialize(
                dispatcher_handle.clone(),
                mls.clone(),
                cancel.clone(),
                client_store,
            )
            .await?;

            let connection = rpc.upstream_connection();
            background_tasks.spawn(async move {
                if let Err(err) = connection.dispatch(message_receiver).await {
                    error!("failed to send messages to server: {:#}", err);
                }
            });

            let client = Arc::new(Self {
                rpc,
                _upstream_address: profile.upstream_address,
                upstream_certificate,
                root_certificate: root_certificate.clone(),
                user_credential: user_credential,
                tunnel_manager,
                mls,
                message_sender: dispatcher_handle.clone(),
                state_handle: client_state_handle,
                background_tasks,
                cancel,
            });

            let connection = client.rpc.upstream_connection();
            let cancel = client.cancel.clone();
            let password = password.clone();
            let user_credential = client.user_credential.clone();
            client.background_tasks.spawn(async move {
                debug!("starting user mls update task");
                let cancel = cancel;
                let password = password;
                let key_retriever = key_retriever;
                let user_credential = user_credential;
                let verifier = remote_verifier;
                loop {
                    if let Err(err) = connection
                        .dispatch(UpdateUserMls {
                            password: password.clone(),
                            key_retriever: key_retriever.clone(),
                            user_credential: user_credential.clone(),
                            verifier: verifier.clone(),
                        })
                        .await
                    {
                        tracing::error!("error while updating user mls: {}", err);
                    }
                    if cancel
                        .run_until_cancelled(tokio::time::sleep(Duration::from_secs(30)))
                        .await
                        .is_none()
                    {
                        break;
                    }
                }
            });

            Ok(client)
        } else {
            Err(anyhow!("Profile is empty"))
        }
    }
}
