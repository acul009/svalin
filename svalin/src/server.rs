use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context, Result, anyhow};
use command_builder::SvalinCommandBuilder;
use config_builder::ServerConfigBuilder;
use openmls_sqlx_storage::SqliteStorageProvider;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Credential, EncryptedCredential, KnownCertificateVerifier, UnverifiedCertificate,
};
use svalin_rpc::{
    permissions::{DummyPermission, anonymous_permission_handler::AnonymousPermissionHandler},
    rpc::{command::handler::HandlerCollection, server::Socket},
    verifiers::skip_verify::SkipClientVerification,
};
use svalin_server_store::{ServerStore, UserStore};
use tokio::{
    sync::{mpsc, oneshot},
    time::{error::Elapsed, timeout},
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{debug, error};

use crate::{
    server::{chain_loader::ChainLoader, local_key_retriever::LocalKeyRetriever},
    shared::commands::{
        init::{InitHandler, ServerInitSuccess},
        public_server_status::{PublicStatus, PublicStatusHandler},
    },
    util::{
        key_storage::KeySource,
        location::{Location, LocationError},
    },
    verifier::{local_verifier::LocalVerifier, tls_optional_wrapper::TlsOptionalWrapper},
};

use svalin_rpc::rpc::server::RpcServer;

pub mod chain_loader;
pub mod command_builder;
pub mod config_builder;
pub mod local_key_retriever;

pub type MlsServer = svalin_pki::mls::server::MlsServer<LocalVerifier, LocalKeyRetriever>;

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
    store_close_handle: svalin_server_store::CloseHandle,
    tasks: TaskTracker,
}

#[derive(Serialize, Deserialize)]
struct BaseConfig {
    root_cert: UnverifiedCertificate,
    credentials: EncryptedCredential,
    key_source: KeySource,
    pseudo_data_seed: Vec<u8>,
}

impl Server {
    pub fn build() -> ServerConfigBuilder<(), ()> {
        config_builder::new()
    }

    async fn data_dir() -> Result<Location, LocationError> {
        Location::system_data_dir()?
            .push("server")
            .ensure_exists()
            .await
    }

    async fn base_config_path() -> Result<Location> {
        Ok(Self::data_dir().await?.push("base_config.json"))
    }

    async fn get_base_config() -> Result<Option<BaseConfig>> {
        let location = Self::base_config_path().await?;
        if tokio::fs::try_exists(&location).await? {
            let config = tokio::fs::read(&location).await?;
            Ok(Some(serde_json::from_slice(&config)?))
        } else {
            Ok(None)
        }
    }

    async fn save_base_config(config: &BaseConfig) -> Result<()> {
        let location = Self::base_config_path().await?;
        let config = serde_json::to_vec_pretty(config)?;
        tokio::fs::write(&location, config).await?;
        Ok(())
    }

    async fn open_mls_server(
        verifier: LocalVerifier,
        key_retriever: LocalKeyRetriever,
    ) -> Result<Arc<MlsServer>> {
        let location = Self::data_dir().await?.push("mls-store.sqlite");
        let storage_provider = SqliteStorageProvider::open(location.as_path()).await?;

        let mls = MlsServer::new(storage_provider, verifier, key_retriever);

        Ok(Arc::new(mls))
    }

    async fn start(config: ServerConfig) -> Result<Self> {
        let base_config = Self::get_base_config()
            .await
            .context("error opening config")?;

        debug!("opening DB");
        let db_path = Self::data_dir().await?.push("db.sqlite");
        tracing::debug!("opening server store at: {}", &db_path);
        let store = ServerStore::open(&db_path)
            .await
            .context("error opending server store")?;

        debug!("creating socket");

        let socket = RpcServer::create_socket(config.addr).context("failed to create socket")?;

        let base_config = match base_config {
            Some(conf) => conf,
            None => {
                // initialize

                debug!("Server is not yet initialized, starting initialization routine");

                let init_success = Self::init_server(
                    socket.clone(),
                    config.cancelation_token.child_token(),
                    store.users.clone(),
                )
                .await
                .context("failed to initialize server")?;

                debug!("Initialisation complete, waiting for init server shutdown");

                // Sleep until the init server has shut down and released the Port
                tokio::time::sleep(INIT_SERVER_SHUTDOWN_COUNTDOWN).await;

                let pseudo_data_seed: Vec<u8> = rand::rng()
                    .sample_iter(rand::distr::StandardUniform)
                    .take(32)
                    .collect();

                let key_source = KeySource::generate_builtin()?;

                let conf = BaseConfig {
                    root_cert: init_success.root.to_unverified(),
                    credentials: key_source
                        .encrypt_credential(&init_success.credential)
                        .await?,
                    pseudo_data_seed,
                    key_source,
                };

                Self::save_base_config(&conf).await?;

                conf
            }
        };

        let root = base_config.root_cert.use_as_root()?;

        let credentials = base_config
            .key_source
            .decrypt_credentials(base_config.credentials)
            .await?;

        let loader = ChainLoader::new(
            store.users.clone(),
            store.agents.clone(),
            store.sessions.clone(),
        );

        let verifier = LocalVerifier::new(root.clone(), loader.clone());

        let key_retriever = LocalKeyRetriever::new(
            root.clone(),
            store.agents.clone(),
            store.users.clone(),
            store.sessions.clone(),
            store.key_packages.clone(),
        );

        let mls = Self::open_mls_server(verifier.clone(), key_retriever).await?;

        let verifier = TlsOptionalWrapper::new(verifier.to_tls_verifier());

        let (to_mls, from_mls) = mpsc::channel(100);

        let command_builder = SvalinCommandBuilder {
            root_cert: root,
            server_cert: credentials.certificate().clone(),
            store,
            mls: mls.clone(),
            to_mls,
        };

        let tasks = TaskTracker::new();

        let message_store = command_builder.store.messages.clone();
        tasks.spawn(async move {
            let mut recv = from_mls;
            while let Some(message) = recv.recv().await {
                match mls.process_message(message).await {
                    Ok(message) => {
                        if let Err(err) = message_store.add_message(message).await {
                            tracing::error!("failed to add message to store: {:#}", err);
                        }
                    }
                    Err(err) => {
                        tracing::error!("failed to process message: {:#}", err);
                    }
                }
            }
        });

        let store_close_handle = command_builder.store.close_handle();

        let rpc = RpcServer::build()
            .credentials(credentials.clone())
            .client_cert_verifier(verifier)
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

        let temp_credentials = Credential::generate_root()?;

        debug!("starting up init server");
        let rpc = RpcServer::build()
            .credentials(temp_credentials)
            .cancellation_token(cancel)
            .client_cert_verifier(SkipClientVerification::new())
            .commands(commands)
            .task_tracker(TaskTracker::new())
            .start_server(socket)
            .await?;

        debug!("init server running");

        if let Ok(result) = recv.await {
            debug!("successfully initialized server");
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
