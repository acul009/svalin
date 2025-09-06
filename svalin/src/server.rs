use std::{net::SocketAddr, sync::Arc, time::Duration};

use agent_store::AgentStore;
use anyhow::{Context, Result, anyhow};
use command_builder::SvalinCommandBuilder;
use config_builder::ServerConfigBuilder;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, migrate::MigrateDatabase, sqlite::SqlitePoolOptions};
use svalin_pki::{Certificate, Credential, EncryptedCredential, KnownCertificateVerifier};
use svalin_rpc::{
    permissions::{DummyPermission, anonymous_permission_handler::AnonymousPermissionHandler},
    rpc::{command::handler::HandlerCollection, server::Socket},
    verifiers::skip_verify::SkipClientVerification,
};
use tokio::{
    sync::{mpsc, oneshot},
    time::{error::Elapsed, timeout},
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{debug, error};

use crate::{
    server::session_store::SessionStore,
    shared::commands::{
        init::{InitHandler, ServerInitSuccess},
        public_server_status::{PublicStatus, PublicStatusHandler},
    },
    util::{key_storage::KeySource, location::Location},
    verifier::{
        incoming_connection_verifier::IncomingConnectionVerifier,
        tls_optional_wrapper::TlsOptionalWrapper, verification_helper::VerificationHelper,
    },
};

use svalin_rpc::rpc::server::RpcServer;

use self::user_store::UserStore;

pub mod agent_store;
pub mod command_builder;
pub mod config_builder;
pub mod session_store;
pub mod user_store;

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
    pool: SqlitePool,
    tasks: TaskTracker,
}

#[derive(Serialize, Deserialize)]
struct BaseConfig {
    root_cert: Certificate,
    credentials: EncryptedCredential,
    key_source: KeySource,
    pseudo_data_seed: Vec<u8>,
}

impl Server {
    pub fn build() -> ServerConfigBuilder<(), ()> {
        config_builder::new()
    }

    async fn data_dir() -> Result<Location> {
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

    async fn open_db() -> Result<SqlitePool> {
        let location = Self::data_dir().await?.push("db.sqlite");
        let url = format!("{}", &location);

        debug!("database_url: {}", &url);

        if !tokio::fs::try_exists(location.as_path()).await? {
            sqlx::Sqlite::create_database(&url).await?;
        }

        let pool = SqlitePoolOptions::new().connect(&url).await?;

        sqlx::migrate!("migrations/server").run(&pool).await?;

        Ok(pool)
    }

    async fn start(config: ServerConfig) -> Result<Self> {
        let base_config = Self::get_base_config()
            .await
            .context("error opening config")?;

        debug!("opening DB");

        let pool = Self::open_db().await.context("error opening db")?;

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
                    pool.clone(),
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
                    root_cert: init_success.root,
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

        let root = base_config.root_cert;

        let user_store = UserStore::open(pool.clone(), root.clone());

        let session_store = SessionStore::open(pool.clone(), user_store.clone());

        let agent_store = AgentStore::open(pool.clone(), root.clone());

        let credentials = base_config
            .key_source
            .decrypt_credentials(base_config.credentials)
            .await?;

        let helper = VerificationHelper::new(root.clone(), user_store.clone());

        let verifier = IncomingConnectionVerifier::new(
            helper,
            root.clone(),
            user_store.clone(),
            session_store.clone(),
            agent_store.clone(),
        )
        .to_tls_verifier();

        let verifier = TlsOptionalWrapper::new(verifier);

        let command_builder = SvalinCommandBuilder {
            root_cert: root.clone(),
            server_cert: credentials.get_certificate().clone(),
            user_store,
            agent_store,
            session_store,
        };

        let tasks = TaskTracker::new();

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
            pool,
        })
    }

    async fn init_server(
        socket: Socket,
        cancel: CancellationToken,
        pool: SqlitePool,
    ) -> Result<ServerInitSuccess> {
        let permission_handler = AnonymousPermissionHandler::<DummyPermission>::default();

        let (send, recv) = oneshot::channel();

        let commands = HandlerCollection::new(permission_handler);
        commands
            .chain()
            .await
            .add(InitHandler::new(send, pool))
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

        self.pool.close().await;

        match result1 {
            Err(e) => Err(e),
            Ok(()) => result2,
        }
    }
}
