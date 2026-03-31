use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use openmls_sqlx_storage::SqliteStorageProvider;
use serde::{Deserialize, Serialize};
use sqlx::migrate::MigrateDatabase;
use sqlx::{Sqlite, SqlitePool};
use svalin_pki::mls::agent::MlsAgent;
use svalin_pki::mls::key_retriever;
use svalin_pki::mls::provider::PostcardCodec;
use svalin_pki::{
    Certificate, Credential, EncryptedCredential, ExactVerififier, KnownCertificateVerifier,
    RootCertificate, UnverifiedCertificate, get_current_timestamp,
};
use svalin_rpc::commands::deauthenticate::DeauthenticateHandler;
use svalin_rpc::commands::e2e::E2EHandler;
use svalin_rpc::commands::ping::PingHandler;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::rpc::command::handler::HandlerCollection;
use svalin_rpc::rpc::connection::Connection;
use tokio::task::JoinSet;
use tokio::time::error::Elapsed;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, instrument};
use update::Updater;
use update::request_available_version::AvailableVersionHandler;
use update::request_installation_info::InstallationInfoHandler;
use update::start_agent_update::StartUpdateHandler;

mod init;
pub mod update;

use crate::client::tunnel_manager::tcp::handler::TcpForwardHandler;
use crate::permissions::default_permission_handler::DefaultPermissionHandler;
use crate::remote_key_retriever::RemoteKeyRetriever;
use crate::shared::commands::realtime_status::RealtimeStatusHandler;
use crate::shared::commands::terminal::RemoteTerminalHandler;
use crate::shared::commands::upload_key_packages::UploadKeyPackages;
use crate::shared::join_agent::AgentInitPayload;
use crate::util::key_storage::KeySource;
use crate::util::location::{Location, LocationError};
use crate::verifier::remote_verifier::RemoteVerifier;

pub struct Agent {
    rpc: Arc<RpcClient>,
    root_certificate: RootCertificate,
    credentials: Credential,
    cancel: CancellationToken,
    mls: MlsAgent<RemoteKeyRetriever, RemoteVerifier>,
    tasks: TaskTracker,
}

impl Agent {
    #[instrument]
    pub async fn run(cancel: CancellationToken) -> Result<()> {
        debug!("opening agent configuration");

        let config = Self::get_config()
            .await
            .context("error loading config")?
            .ok_or_else(|| anyhow!("agent is not yet initialized"))?;

        debug!("decrypting agent credentials");

        let credentials = config
            .key_source
            .decrypt_credentials(config.encrypted_credentials)
            .await
            .context("error decrypting credentials")?;

        debug!("building upstream verifier");
        let root_certificate = config.root_certificate.use_as_root()?;

        let upstream_certificate = config
            .upstream_certificate
            .verify_signature(&root_certificate, get_current_timestamp())
            .context("error verifying upstream certificate")?;

        let verifier = ExactVerififier::new(upstream_certificate).to_tls_verifier();

        debug!("trying to connect to server");

        let rpc = RpcClient::connect(
            &config.upstream_address,
            Some(&credentials),
            verifier,
            cancel.clone(),
        )
        .await
        .context("error connecting rpc")?;

        debug!("connection to server established");

        let storage_provider = Self::open_mls_store().await?;

        let key_retriever =
            RemoteKeyRetriever::new(rpc.upstream_connection(), root_certificate.clone());

        let verifier = RemoteVerifier::new(root_certificate.clone(), rpc.upstream_connection());

        let mls = Arc::new(
            MlsAgent::new(
                credentials.clone(),
                storage_provider,
                key_retriever,
                verifier,
            )
            .await?,
        );

        let permission_handler = DefaultPermissionHandler::new(root_certificate.clone());

        let e2e_commands = HandlerCollection::new(permission_handler.clone());

        let updater = Updater::new(cancel.clone())
            .await
            .context("error creating updater")?;

        e2e_commands
            .chain()
            .await
            .add(PingHandler)
            .add(RealtimeStatusHandler)
            .add(RemoteTerminalHandler)
            .add(TcpForwardHandler)
            .add(InstallationInfoHandler::new(updater.clone()))
            .add(AvailableVersionHandler::new(updater.clone()))
            .add(StartUpdateHandler::new(updater));

        let public_commands = HandlerCollection::new(permission_handler.clone());

        let verifier =
            RemoteVerifier::new(root_certificate.clone(), rpc.upstream_connection()).session_only();

        public_commands.chain().await.add(E2EHandler::new(
            credentials.clone(),
            e2e_commands,
            verifier.to_tls_verifier(),
        ));

        let server_commands = HandlerCollection::new(permission_handler);

        server_commands
            .chain()
            .await
            .add(DeauthenticateHandler::new(public_commands));

        debug!("Starting agent background tasks");

        let connection = rpc.upstream_connection();
        let mls2 = mls.clone();
        let key_package_task = tokio::spawn(async move {
            connection.dispatch(UploadKeyPackages())
        });

        debug!("Agent will now start serving requests");

        rpc.serve(server_commands)
            .await
            .context("error serving rpc")?;

        key_package_task.await;

        Ok(())
    }

    pub async fn init_with(data: AgentInitPayload) -> Result<()> {
        let key_source = KeySource::generate_builtin()?;

        let config = AgentConfig {
            root_certificate: data.root.to_unverified(),
            upstream_certificate: data.upstream.to_unverified(),
            encrypted_credentials: key_source.encrypt_credential(&data.credentials).await?,
            upstream_address: data.address,
            key_source,
        };

        if Self::get_config().await?.is_some() {
            return Err(anyhow!("Agent configuration already exists"));
        }

        Self::save_config(&config).await?;

        Ok(())
    }

    async fn data_dir() -> Result<Location, LocationError> {
        Location::system_data_dir()?
            .push("agent")
            .ensure_exists()
            .await
    }

    async fn config_path() -> Result<Location, LocationError> {
        Ok(Self::data_dir().await?.push("config.json"))
    }

    async fn get_config() -> Result<Option<AgentConfig>> {
        let location = Self::config_path().await?;
        if tokio::fs::try_exists(&location).await? {
            let config = tokio::fs::read(&location).await?;
            Ok(Some(serde_json::from_slice(&config)?))
        } else {
            Ok(None)
        }
    }

    async fn mls_db_path() -> Result<Location, LocationError> {
        Ok(Self::data_dir().await?.push("mls-store.sqlite"))
    }

    async fn open_mls_store() -> Result<SqliteStorageProvider<PostcardCodec>, OpenMlsStoreError> {
        let location = Self::mls_db_path().await?;

        let path = location
            .as_path()
            .to_str()
            .ok_or_else(|| OpenMlsStoreError::LocationBroken)?;

        if !location.exists().await {
            Sqlite::create_database(path).await?;
        }

        let pool = SqlitePool::connect(path).await?;
        let provider = SqliteStorageProvider::new(pool);
        provider.run_migrations().await?;

        Ok(provider)
    }

    async fn save_config(config: &AgentConfig) -> Result<()> {
        let location = Self::config_path().await?;
        let config = serde_json::to_vec_pretty(config)?;
        tokio::fs::write(&location, config).await?;
        Ok(())
    }

    pub async fn close(&mut self, timeout_duration: Duration) -> Result<(), Elapsed> {
        self.cancel.cancel();
        let result1 = self.rpc.close(timeout_duration).await;
        self.tasks.wait().await;

        result1
    }
}

#[derive(Serialize, Deserialize)]
struct AgentConfig {
    upstream_address: String,
    upstream_certificate: UnverifiedCertificate,
    root_certificate: UnverifiedCertificate,
    encrypted_credentials: EncryptedCredential,
    key_source: KeySource,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateMlsStoreError {
    #[error("found an existing mls data store")]
    AlreadyExists,
    #[error("error getting store location: {0}")]
    LocationError(#[from] LocationError),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("location did not give valid UTF-8 string")]
    LocationBroken,
    #[error("sqlx error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("sqlx migration error: {0}")]
    MigrateError(#[from] sqlx::migrate::MigrateError),
}

#[derive(Debug, thiserror::Error)]
pub enum OpenMlsStoreError {
    #[error("error getting store location: {0}")]
    LocationError(#[from] LocationError),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("location did not give valid UTF-8 string")]
    LocationBroken,
    #[error("sqlx error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("sqlx migration error: {0}")]
    MigrateError(#[from] sqlx::migrate::MigrateError),
}
