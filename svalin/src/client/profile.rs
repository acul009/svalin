use std::sync::Arc;

use anyhow::{Result, anyhow};
use openmls_sqlx_storage::SqliteStorageProvider;
use serde::{Deserialize, Serialize};
use svalin_client_store::ClientStore;
use svalin_pki::{
    ArgonParams, Certificate, Credential, EncryptedCredential, ExactVerififier,
    KnownCertificateVerifier, RootCertificate, UnverifiedCertificate, get_current_timestamp,
    mls::client::MlsClient,
};
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::error;

use crate::{
    client::tunnel_manager::TunnelManager,
    message_streaming::client::{ClientMessageDispatcher, ClientMessageReceiver},
    remote_key_retriever::RemoteKeyRetriever,
    shared::commands::{get_user_credentials::GetUserCredential, update_user_mls::UpdateUserMls},
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
    pub(crate) local_credential_params: ArgonParams,
    pub(crate) device_credential: EncryptedCredential,
}

impl Profile {
    pub(crate) fn new(
        username: String,
        upstream_address: String,
        upstream_certificate: Certificate,
        root_certificate: RootCertificate,
        local_credential_params: ArgonParams,
        device_credential: EncryptedCredential,
    ) -> Self {
        Self {
            username,
            upstream_address,
            upstream_certificate: upstream_certificate.to_unverified(),
            root_certificate: root_certificate.to_unverified(),
            local_credential_params,
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
        device_credentials: Credential,
        password: Vec<u8>,
    ) -> Result<String> {
        let local_credential_params = ArgonParams::strong();
        let key = local_credential_params
            .derive_encryption_key(password)
            .await?;
        let encrypted_device_credential = device_credentials.export(&key)?;

        let profile = Profile::new(
            username,
            upstream_address,
            upstream_certificate,
            root_certificate,
            local_credential_params,
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

    pub async fn open_profile(
        profile_key: &str,
        password: Vec<u8>,
        cancel: CancellationToken,
    ) -> Result<Arc<Self>> {
        let Some(profile) = Self::get_profile(&profile_key).await? else {
            return Err(anyhow!("Profile is empty"));
        };

        let key = profile
            .local_credential_params
            .derive_encryption_key(password.clone())
            .await?;

        let mls_db_path = profile.profile_dir().await?.push("mls-store.sqlite");
        let client_db_path = profile.profile_dir().await?.push("client-store.sqlite");
        // tracing::trace!("unlocking profile");
        let device_credential = profile.device_credential.decrypt(&key)?;
        let root_certificate = profile.root_certificate.use_as_root()?;
        let upstream_certificate = profile
            .upstream_certificate
            .verify_signature(&root_certificate, get_current_timestamp())?;

        // tracing::trace!("creating verifier");
        let verifier = ExactVerififier::new(upstream_certificate.clone()).to_tls_verifier();

        // tracing::trace!("connecting to server");
        let rpc = RpcClient::connect(
            &profile.upstream_address,
            Some(&device_credential),
            verifier,
            cancel.clone(),
        )
        .await?;

        let user_credential = rpc
            .upstream_connection()
            .dispatch(GetUserCredential)
            .await
            .map_err(|err| anyhow!(err))?;
        let key = user_credential
            .params
            .derive_encryption_key(password)
            .await?;
        let user_credential = user_credential.credential.decrypt(&key)?;

        let remote_verifier =
            RemoteVerifier::new(root_certificate.clone(), rpc.upstream_connection());

        // tracing::trace!("connected to server");

        // tracing::trace!("opening sqlite database: {}", db_path.display());
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

        let connection = rpc.upstream_connection();
        background_tasks.spawn(async move {
            if let Err(err) = connection.dispatch(message_dispatcher).await {
                error!("failed to send messages to server: {:#}", err);
            }
        });

        let client_store = Arc::new(ClientStore::open(client_db_path).await?);

        // Initialize the client message receiver
        let (message_receiver, client_state_handle) = ClientMessageReceiver::initialize(
            dispatcher_handle.clone(),
            mls.clone(),
            cancel.clone(),
            client_store,
        )
        .await?;
        // and start it
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
            user_credential,
            device_credential,
            verifier: remote_verifier.clone(),
            tunnel_manager,
            mls: mls.clone(),
            message_sender: dispatcher_handle.clone(),
            state_handle: client_state_handle,
            background_tasks,
            cancel,
        });

        let connection = client.rpc.upstream_connection();
        let cancel = client.cancel.clone();
        let user_credential = client.user_credential.clone();
        let session_mls = mls.clone();
        let state_handle = client.state_handle.clone();
        client.background_tasks.spawn(async move {
            tracing::trace!("starting user mls update task");
            let verifier = remote_verifier;
            if let Err(err) = connection
                .dispatch(UpdateUserMls {
                    key: key,
                    key_retriever: key_retriever,
                    user_credential: user_credential,
                    verifier: verifier,
                    session_mls: session_mls,
                    cancel,
                    state_handle,
                })
                .await
            {
                tracing::error!("failed to update user mls: {}", err);
            }
        });

        Ok(client)
    }
}
