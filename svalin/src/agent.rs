use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, anyhow};
use openmls_sqlx_storage::SqliteStorageProvider;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Credential, EncryptedCredential, ExactVerififier, KnownCertificateVerifier, RootCertificate,
    UnverifiedCertificate, get_current_timestamp, mls::provider::PostcardCodec,
};
use svalin_rpc::{
    commands::{deauthenticate::DeauthenticateHandler, e2e::E2EHandler, ping::PingHandler},
    rpc::{
        client::RpcClient,
        command::handler::HandlerCollection,
        connection::{Connection, ServeableConnectionBase},
    },
};
use tokio::{sync::Notify, time::error::Elapsed};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::instrument;
use update::{
    Updater, request_available_version::AvailableVersionHandler,
    request_installation_info::InstallationInfoHandler, start_agent_update::StartUpdateHandler,
};

mod init;
mod mls;
pub mod update;

use crate::shared::commands::terminal::RemoteTerminalHandler;
use crate::shared::commands::{
    realtime_status::RealtimeStatusHandler, request_system_report::RequestSystemReportHandler,
};
use crate::shared::join_agent::AgentInitPayload;
use crate::util::key_storage::KeySource;
use crate::util::location::{Location, LocationError};
use crate::verifier::remote_verifier::RemoteVerifier;
use crate::{
    client::tunnel_manager::tcp::handler::TcpForwardHandler,
    message_streaming::agent::AgentMessageDispatcher,
};
use crate::{
    message_streaming::agent::AgentMessageReceiver,
    permissions::default_permission_handler::DefaultPermissionHandler,
};
use crate::{mls::MlsAgent, remote_key_retriever::RemoteKeyRetriever};

pub struct Agent {
    rpc: Arc<RpcClient>,
    root_certificate: RootCertificate,
    credentials: Credential,
    cancel: CancellationToken,
    mls: MlsAgent,
    tasks: TaskTracker,
}

impl Agent {
    #[instrument]
    pub async fn run(cancel: CancellationToken) -> Result<()> {
        tracing::trace!("opening agent configuration");

        let config = Self::get_config()
            .await
            .context("error loading config")?
            .ok_or_else(|| anyhow!("agent is not yet initialized"))?;

        // tracing::trace!("decrypting agent credentials");

        let credentials = config
            .key_source
            .decrypt_credentials(config.encrypted_credentials)
            .await
            .context("error decrypting credentials")?;

        // tracing::trace!("building upstream verifier");
        let root_certificate = config.root_certificate.use_as_root()?;

        let upstream_certificate = config
            .upstream_certificate
            .verify_signature(&root_certificate, get_current_timestamp())
            .context("error verifying upstream certificate")?;

        let verifier = ExactVerififier::new(upstream_certificate).to_tls_verifier();

        tracing::trace!("trying to connect to server");

        let rpc = RpcClient::connect(
            &config.upstream_address,
            Some(&credentials),
            verifier,
            cancel.clone(),
        )
        .await
        .context("error connecting rpc")?;

        tracing::trace!("connection to server established");

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

        let system_report_notify = Arc::new(Notify::new());

        e2e_commands
            .chain()
            .await
            .add(PingHandler)
            .add(RealtimeStatusHandler)
            .add(RemoteTerminalHandler)
            .add(TcpForwardHandler)
            .add(InstallationInfoHandler::new(updater.clone()))
            .add(AvailableVersionHandler::new(updater.clone()))
            .add(StartUpdateHandler::new(updater))
            .add(RequestSystemReportHandler {
                notify: system_report_notify.clone(),
            });

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

        tracing::trace!("Starting agent background tasks");
        let tasks = TaskTracker::new();

        let (messager_handle, message_dispatcher) = AgentMessageDispatcher::new();

        let connection = rpc.upstream_connection();
        tasks.spawn(async move {
            if let Err(err) = connection.dispatch(message_dispatcher).await {
                tracing::error!("Failed to dispatch messages: {err}");
            }
        });

        let receiver = AgentMessageReceiver {
            cancel: cancel.clone(),
            mls: mls.clone(),
            sender: messager_handle.clone(),
        };

        let connection = rpc.upstream_connection();
        tasks.spawn(async move {
            if let Err(err) = connection.dispatch(receiver).await {
                tracing::error!("Failed to receive messages: {err}");
            }
        });

        let connection = rpc.upstream_connection();
        tasks.spawn(async move {
            tracing::trace!("Agent will now start serving requests");
            if let Err(err) = rpc.serve(server_commands).await {
                tracing::error!("Failed to serve requests: {err}");
            }
        });

        mls::ensure_group_exists(&mls, &messager_handle).await?;

        tasks.spawn(mls::schedule_system_reports(
            mls,
            messager_handle,
            cancel.clone(),
            system_report_notify,
        ));

        cancel.cancelled().await;
        tasks.close();
        // TODO: add timeout with error message
        tokio::time::timeout(Duration::from_secs(3), tasks.wait()).await?;

        ServeableConnectionBase::close(&connection).await;

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

    pub fn data_dir() -> Result<Location, LocationError> {
        Ok(Location::system_data_dir()?.push("agent"))
    }

    fn config_path() -> Result<Location, LocationError> {
        Ok(Self::data_dir()?.push("config.json"))
    }

    async fn get_config() -> Result<Option<AgentConfig>> {
        let location = Self::config_path()?;
        if tokio::fs::try_exists(&location).await? {
            let config = tokio::fs::read(&location).await?;
            Ok(Some(serde_json::from_slice(&config)?))
        } else {
            Ok(None)
        }
    }

    fn mls_db_path() -> Result<Location, LocationError> {
        Ok(Self::data_dir()?.push("mls-store.sqlite"))
    }

    async fn open_mls_store() -> Result<SqliteStorageProvider<PostcardCodec>, OpenMlsStoreError> {
        let location = Self::mls_db_path()?;

        let path = location
            .as_path()
            .to_str()
            .ok_or_else(|| OpenMlsStoreError::LocationBroken)?;

        Ok(SqliteStorageProvider::open(path).await?)
    }

    async fn save_config(config: &AgentConfig) -> Result<()> {
        let location = Self::config_path()?.ensure_parent_exists().await?;
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
    StoreError(#[from] openmls_sqlx_storage::Error),
}
