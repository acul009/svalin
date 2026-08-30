use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use svalin_pki::{
    ArgonParams, Certificate, Credential, EncryptedCredential, ExactVerififier, RootCertificate,
    TrustStoreVerifier, UnverifiedCertificate, Verifier, get_current_timestamp,
    trust_store::TrustStore,
};
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};
use svalin_store::client_store::ClientStore;
use tokio::sync::{mpsc, oneshot};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::error;

use crate::{
    client::tunnel_manager::TunnelManager,
    message_streaming::client::{ClientMessageDispatcher, ClientMessageReceiver},
    remote_key_retriever::RemoteKeyRetriever,
    shared::commands::{get_user_credentials::GetUserCredential, update_mls::UpdateMls},
    util::{
        location::{Location, LocationError},
        trust_store::{load_trust_store, save_trust_store, update_trust_store},
    },
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

    pub async fn trust_store_path(&self) -> Result<Location> {
        Ok(self.profile_dir().await?.push("trust_store.json"))
    }
}

impl Client {
    async fn data_dir() -> Result<Location, LocationError> {
        Location::user_data_dir()?
            .push("client")
            .ensure_parent_exists()
            .await
    }

    async fn profile_dir(profile_name: &str) -> Result<Location> {
        Ok(Self::data_dir().await?.push(profile_name.replace(":", "+")))
    }

    pub async fn list_profiles() -> Result<Vec<String>> {
        let location = Self::data_dir().await?;

        let mut folders = match tokio::fs::read_dir(&location).await {
            Ok(folders) => folders,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(e) => return Err(e.into()),
        };

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
        trust_store: &TrustStore,
    ) -> Result<String> {
        let local_credential_params = ArgonParams::strong();
        let key = local_credential_params
            .derive_encryption_key(password)
            .await
            .context("failed to derive encryption key")?;
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

        if Self::list_profiles()
            .await
            .context("failed to list profiles")?
            .contains(&profile_name)
        {
            return Err(anyhow!("profile already exists"));
        }

        Self::save_profile(&profile)
            .await
            .context("failed to save profile to disk")?;

        let trust_store_path = profile.trust_store_path().await?;
        save_trust_store(&trust_store_path, &trust_store.export()).await?;

        Ok(profile_name)
    }

    async fn save_profile(profile: &Profile) -> Result<()> {
        let location = Self::profile_dir(&profile.name())
            .await?
            .push("profile.json")
            .ensure_parent_exists()
            .await?;

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

        // let mls_db_path = profile.profile_dir().await?.push("mls-store.sqlite");
        let client_db_path = profile.profile_dir().await?.push("client-store.sqlite");
        let trust_store_path = profile.trust_store_path().await?.to_pathbuf();
        // tracing::trace!("unlocking profile");
        let device_credential = profile.device_credential.decrypt(&key)?;
        let root_certificate = profile.root_certificate.use_as_root()?;
        let upstream_certificate = profile
            .upstream_certificate
            .verify_signature(&root_certificate, get_current_timestamp())?;

        let client_store = Arc::new(ClientStore::open(client_db_path).await?);
        // Starting Background Tasks
        let background_tasks = TaskTracker::new();
        let trust_store = load_trust_store(
            trust_store_path,
            client_store.transaction_store().as_ref(),
            cancel.clone(),
            &background_tasks,
        )
        .await
        .context("Failed to load trust store")?;

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

        update_trust_store(
            trust_store.clone(),
            client_store.transaction_store().clone(),
            rpc.upstream_connection(),
            cancel.clone(),
            &background_tasks,
        )
        .await?;

        let verifier = TrustStoreVerifier::new(trust_store.clone());

        // tracing::trace!("connected to server");

        // tracing::trace!("opening sqlite database: {}", db_path.display());
        // let url = mls_db_path
        //     .as_path()
        //     .to_str()
        //     .ok_or_else(|| anyhow!("db_path was not valid UTF-8"))?;
        // let storage_provider = SqliteStorageProvider::open(&url).await?;
        let key_retriever = RemoteKeyRetriever::new(rpc.upstream_connection(), trust_store.clone());

        // let mls = Arc::new(MlsClient::new(
        //     device_credential.clone(),
        //     storage_provider.into(),
        //     key_retriever.clone(),
        //     verifier.clone(),
        // )?);

        let tunnel_manager = TunnelManager::new();

        let (dispatcher_handle, message_dispatcher) = ClientMessageDispatcher::new();

        let connection = rpc.upstream_connection();
        background_tasks.spawn(async move {
            if let Err(err) = connection.dispatch(message_dispatcher).await {
                error!("failed to send messages to server: {:#}", err);
            }
        });

        // Initialize the client message receiver
        let (message_receiver, client_state_handle) = ClientMessageReceiver::initialize(
            dispatcher_handle.clone(),
            // mls.clone(),
            cancel.clone(),
            client_store.clone(),
        )
        .await?;
        // and start it
        let connection = rpc.upstream_connection();
        background_tasks.spawn(async move {
            if let Err(err) = connection.dispatch(message_receiver).await {
                error!("failed to send messages to server: {:#}", err);
            }
        });

        let connection = rpc.upstream_connection();
        let cancel2 = cancel.clone();
        let user_credential2 = user_credential.clone();
        let state_handle = client_state_handle.clone();
        let verifier2 = verifier.clone();
        let (mls_update_sender, mls_updates) = mpsc::channel(10);
        let (up_to_date_send, up_to_date_recv) = oneshot::channel();
        background_tasks.spawn(async move {
            tracing::trace!("starting user mls update task");
            if let Err(err) = connection
                .dispatch(UpdateMls {
                    mls_updates,
                    key_retriever: key_retriever,
                    user_credential: user_credential2,
                    encryption_key: key,
                    verifier: verifier2,
                    client_state: state_handle,
                    cancel: cancel2,
                    up_to_date: up_to_date_send,
                })
                .await
            {
                tracing::error!("failed to update user mls: {}", err);
            }
        });

        up_to_date_recv.await?;

        let client = Arc::new(Self {
            rpc,
            _upstream_address: profile.upstream_address,
            upstream_certificate,
            root_certificate: root_certificate.clone(),
            user_credential,
            device_credential,
            verifier: verifier.clone(),
            tunnel_manager,
            trust_store: trust_store,
            store: client_store,
            mls_update_sender,
            message_sender: dispatcher_handle.clone(),
            state_handle: client_state_handle,
            background_tasks,
            cancel,
        });

        Ok(client)
    }
}
