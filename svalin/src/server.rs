use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use command_builder::SvalinCommandBuilder;
use config_builder::ServerConfigBuilder;
use openmls_sqlx_storage::SqliteStorageProvider;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Credential, EncryptedCredential, KnownCertificateVerifier, TrustStoreVerifier,
    trust_store::{self, TrustStore},
};
use svalin_rpc::{
    permissions::{DummyPermission, anonymous_permission_handler::AnonymousPermissionHandler},
    rpc::{command::handler::HandlerCollection, server::Socket},
    verifiers::skip_verify::SkipClientVerification,
};
use svalin_store::server_store::{ServerStore, UserStore};
use tokio::{
    sync::oneshot,
    time::{error::Elapsed, timeout},
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::error;

use crate::{
    server::local_key_retriever::LocalKeyRetriever,
    shared::commands::{
        init::{InitHandler, ServerInitSuccess},
        public_server_status::{PublicStatus, PublicStatusHandler},
    },
    util::{
        key_storage::KeySource,
        location::{Location, LocationError},
    },
    verifier::tls_optional_wrapper::TlsOptionalWrapper,
};

use svalin_rpc::rpc::server::RpcServer;

pub mod chain_loader;
pub mod command_builder;
pub mod config_builder;
pub mod local_key_retriever;

pub type MlsServer = svalin_pki::mls::server::MlsServer<LocalKeyRetriever, TrustStoreVerifier>;

#[derive(Debug)]
pub struct ServerConfig {
    addr: SocketAddr,
    cancelation_token: CancellationToken,
}

pub const INIT_SERVER_SHUTDOWN_COUNTDOWN: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct Server {
    rpc: Arc<RpcServer>,
    config: ServerConfig,
    store_close_handle: svalin_store::CloseHandle,
    tasks: TaskTracker,
}

#[derive(Serialize, Deserialize)]
struct SavedConfig {
    credential: EncryptedCredential,
    key_source: KeySource,
    pseudo_data_seed: Vec<u8>,
}

struct BaseConfig {
    trust_store: Arc<RwLock<TrustStore>>,
    credential: Credential,
    pseudo_data_seed: Vec<u8>,
}

impl Server {
    pub fn build() -> ServerConfigBuilder<(), ()> {
        config_builder::new()
    }

    fn data_dir() -> Result<Location, LocationError> {
        Ok(Location::system_data_dir()?.push("server"))
    }

    fn base_config_path() -> Result<Location, LocationError> {
        Ok(Self::data_dir()?.push("base_config.json"))
    }

    fn trust_store_path() -> Result<Location, LocationError> {
        Ok(Self::data_dir()?.push("trust_store.json"))
    }

    async fn get_base_config() -> anyhow::Result<Option<BaseConfig>> {
        let location = Self::base_config_path()?.ensure_parent_exists().await?;
        if tokio::fs::try_exists(&location).await? {
            let config = tokio::fs::read(&location).await?;
            let config: SavedConfig = serde_json::from_slice(&config)?;
            let credential = config
                .key_source
                .decrypt_credentials(config.credential)
                .await?;

            let trust_store = tokio::fs::read(Self::trust_store_path()?).await?;
            let trust_store: trust_store::Exported =
                serde_json::from_slice(trust_store.as_slice())?;
            let trust_store = TrustStore::import(trust_store)?;
            let trust_store = Arc::new(RwLock::new(trust_store));

            let config = BaseConfig {
                credential,
                trust_store,
                pseudo_data_seed: config.pseudo_data_seed,
            };
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    async fn save_base_config(config: &BaseConfig) -> Result<()> {
        let location = Self::base_config_path()?;
        let key_source = KeySource::generate_builtin()?;
        let trust_store = config.trust_store.read().unwrap().export();
        let config = SavedConfig {
            credential: key_source.encrypt_credential(&config.credential).await?,
            key_source,
            pseudo_data_seed: config.pseudo_data_seed.clone(),
        };
        let config = serde_json::to_vec_pretty(&config)?;
        tokio::fs::write(&location, config).await?;
        let trust_store = serde_json::to_vec_pretty(&trust_store)?;
        tokio::fs::write(&Self::trust_store_path()?, trust_store).await?;
        Ok(())
    }

    async fn open_mls_server(
        verifier: TrustStoreVerifier,
        key_retriever: LocalKeyRetriever,
    ) -> Result<Arc<MlsServer>> {
        let location = Self::data_dir()?.push("mls-store.sqlite");
        let storage_provider = SqliteStorageProvider::open(location.as_path()).await?;

        let mls = MlsServer::new(storage_provider, verifier, key_retriever);

        Ok(Arc::new(mls))
    }

    async fn start(config: ServerConfig) -> Result<Self> {
        let base_config = Self::get_base_config()
            .await
            .context("error opening config")?;

        // tracing::trace!("opening DB");
        let db_path = Self::data_dir()?.push("db.sqlite");
        tracing::trace!("opening server db: {db_path}");
        // tracing::trace!("opening server store at: {}", &db_path);
        let store = ServerStore::open(&db_path)
            .await
            .context("error opending server store")?;

        // tracing::trace!("creating socket");

        let socket = RpcServer::create_socket(config.addr).context("failed to create socket")?;

        let base_config = match base_config {
            Some(conf) => conf,
            None => {
                // initialize

                tracing::trace!("Server is not yet initialized, starting initialization routine");

                let init_success = Self::init_server(
                    socket.clone(),
                    config.cancelation_token.child_token(),
                    store.users.clone(),
                )
                .await
                .context("failed to initialize server")?;

                tracing::trace!("Initialisation complete, waiting for init server shutdown");

                // Sleep until the init server has shut down and released the Port
                tokio::time::sleep(INIT_SERVER_SHUTDOWN_COUNTDOWN).await;

                let pseudo_data_seed: Vec<u8> = rand::rng()
                    .sample_iter(rand::distr::StandardUniform)
                    .take(32)
                    .collect();

                let conf = BaseConfig {
                    trust_store: Arc::new(RwLock::new(init_success.trust_store)),
                    credential: init_success.credential,
                    pseudo_data_seed,
                };

                Self::save_base_config(&conf).await?;

                conf
            }
        };

        let trust_store = base_config.trust_store;
        let root = trust_store.read().unwrap().root().clone();

        let credentials = base_config.credential;

        let verifier = TrustStoreVerifier::new(trust_store.clone());

        let key_retriever = LocalKeyRetriever::new(
            root.clone(),
            trust_store.clone(),
            store.key_packages.clone(),
        );

        let mls = Self::open_mls_server(verifier.clone(), key_retriever).await?;

        let tls_verifier = TlsOptionalWrapper::new(verifier.clone().to_tls_verifier());

        let command_builder = SvalinCommandBuilder {
            trust_store: trust_store.clone(),
            root_cert: root,
            server_cert: credentials.certificate().clone(),
            store,
            mls: mls.clone(),
        };

        let tasks = TaskTracker::new();

        let store_close_handle = command_builder.store.close_handle();

        let rpc = RpcServer::build()
            .credentials(credentials.clone())
            .client_cert_verifier(tls_verifier)
            .cancellation_token(config.cancelation_token.clone())
            .commands(command_builder)
            .task_tracker(tasks.clone())
            .start_server(socket)
            .await?;

        Ok(Self {
            config,
            rpc,
            tasks,
            store_close_handle,
        })
    }

    async fn init_server(
        socket: Socket,
        cancel: CancellationToken,
        user_store: Arc<UserStore>,
    ) -> Result<ServerInitSuccess> {
        let permission_handler = AnonymousPermissionHandler::<DummyPermission>::default();

        let (send, recv) = oneshot::channel();

        let commands = HandlerCollection::new(permission_handler);
        commands
            .chain()
            .await
            .add(InitHandler::new(send, user_store))
            .add(PublicStatusHandler::new(PublicStatus::WaitingForInit));

        let temp_credentials = Credential::generate_temporary()?;

        tracing::trace!("starting up init server");
        let rpc = RpcServer::build()
            .credentials(temp_credentials)
            .cancellation_token(cancel)
            .client_cert_verifier(SkipClientVerification::new())
            .commands(commands)
            .task_tracker(TaskTracker::new())
            .start_server(socket)
            .await?;

        tracing::trace!("init server running");

        if let Ok(result) = recv.await {
            tracing::trace!("successfully initialized server");
            rpc.close(Duration::from_secs(1)).await?;
            Ok(result)
        } else {
            error!("error when trying to initialize server");
            rpc.close(Duration::from_secs(1)).await?;
            Err(anyhow!("error initializing server"))
        }
    }

    pub async fn close(&self, timeout_duration: Duration) -> Result<(), Elapsed> {
        self.config.cancelation_token.cancel();
        let result1 = self.rpc.close(timeout_duration).await;

        self.tasks.close();

        let result2 = timeout(timeout_duration, self.tasks.wait()).await;

        self.store_close_handle.close().await;

        result1.or(result2)
    }
}
