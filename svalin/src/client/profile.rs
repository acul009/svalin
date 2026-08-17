use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use openmls_sqlx_storage::SqliteStorageProvider;
use serde::{Deserialize, Serialize};
use svalin_client_store::ClientStore;
use svalin_pki::{
    ArgonParams, Certificate, Credential, EncryptedCredential, ExactVerififier,
    KnownCertificateVerifier, RootCertificate, TrustStoreVerifier, UnverifiedCertificate,
    get_current_timestamp,
    mls::client::MlsClient,
    trust_store::{self, TrustStore},
};
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};
use tokio::sync::oneshot;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::error;

use crate::{
    client::tunnel_manager::TunnelManager,
    message_streaming::client::{ClientMessageDispatcher, ClientMessageReceiver},
    remote_key_retriever::RemoteKeyRetriever,
    shared::commands::{
        get_user_credentials::GetUserCredential, update_trust_store::UpdateTrustStore,
        update_user_mls::UpdateUserMls,
    },
    util::location::{Location, LocationError},
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
            .ensure_parent_exists()
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

        let mls_db_path = profile.profile_dir().await?.push("mls-store.sqlite");
        let client_db_path = profile.profile_dir().await?.push("client-store.sqlite");
        // tracing::trace!("unlocking profile");
        let device_credential = profile.device_credential.decrypt(&key)?;
        let root_certificate = profile.root_certificate.use_as_root()?;
        let upstream_certificate = profile
            .upstream_certificate
            .verify_signature(&root_certificate, get_current_timestamp())?;

        let client_store = Arc::new(ClientStore::open(client_db_path).await?);
        // Starting Background Tasks
        let background_tasks = TaskTracker::new();
        let trust_store = Self::load_trust_store(
            profile_key,
            client_store.transaction_store(),
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

        let connection = rpc.upstream_connection();
        let trust_store2 = trust_store.clone();
        let store = client_store.clone();
        let (send_ready, trust_store_ready) = oneshot::channel();
        let cancel2 = cancel.clone();
        background_tasks.spawn(async move {
            if let Err(err) = connection
                .dispatch(UpdateTrustStore::new(
                    trust_store2,
                    store,
                    send_ready,
                    cancel2,
                ))
                .await
            {
                eprintln!("Error updating trust store: {}", err);
            }
        });

        // Wait until trust store has been updated to the newest version.
        trust_store_ready.await?;
        let verifier = TrustStoreVerifier::new(trust_store.clone());

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
            verifier.clone(),
        )?);

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
            mls.clone(),
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

        let client = Arc::new(Self {
            rpc,
            _upstream_address: profile.upstream_address,
            upstream_certificate,
            root_certificate: root_certificate.clone(),
            user_credential,
            device_credential,
            verifier: verifier.clone(),
            tunnel_manager,
            mls: mls.clone(),
            trust_store: trust_store,
            store: client_store,
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
            let verifier = verifier;
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

    async fn load_trust_store(
        profile_key: &str,
        store: &svalin_client_store::trust_store_transaction_store::TrustStoreTransactionStore,
        cancel: CancellationToken,
        task_tracker: &TaskTracker,
    ) -> anyhow::Result<Arc<RwLock<TrustStore>>> {
        let location = Self::profile_dir(profile_key).await?;
        let exported = tokio::fs::read(&location).await?;
        let exported: trust_store::Exported = serde_json::from_slice(&exported)?;

        let mut trust_store = TrustStore::import(exported)?;
        let transactions = store.load_all_after(trust_store.sequence()).await?;

        for block in transactions {
            let block = trust_store.check(block)?;
            trust_store.apply(block);
        }

        let exported = trust_store.export();
        let exported = serde_json::to_vec_pretty(&exported)?;
        tokio::fs::write(&location, exported).await?;

        let trust_store = Arc::new(RwLock::new(trust_store));
        let trust_store_2 = trust_store.clone();

        task_tracker.spawn(async move {
            let trust_store = trust_store_2;
            let mut last_digest = trust_store.read().unwrap().digest();
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(300)) => {
                        let exported = {
                            let guard = trust_store.read().unwrap();
                            if guard.digest() == last_digest {
                                continue;
                            }
                            last_digest = guard.digest();
                            guard.export()
                        };
                        let Ok(exported) = serde_json::to_vec_pretty(&exported) else {
                            eprintln!("Failed to serialize trust store");
                            continue;
                        };
                        if let Err(e) = tokio::fs::write(&location, exported).await {
                            eprintln!("Failed to write trust store: {}", e);
                        }
                    }
                    _ = cancel.cancelled() => {
                        break;
                    }
                }
            }
        });

        Ok(trust_store)
    }
}
