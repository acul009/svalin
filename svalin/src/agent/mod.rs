use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Certificate, Credential, EncryptedCredential, ExactVerififier, KnownCertificateVerifier,
};
use svalin_rpc::commands::deauthenticate::DeauthenticateHandler;
use svalin_rpc::commands::e2e::E2EHandler;
use svalin_rpc::commands::ping::PingHandler;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::rpc::command::handler::HandlerCollection;
use tokio::time::error::Elapsed;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};
use update::Updater;
use update::request_available_version::AvailableVersionHandler;
use update::request_installation_info::InstallationInfoHandler;
use update::start_agent_update::StartUpdateHandler;

mod init;
pub mod update;

use crate::client::tunnel_manager::tcp::handler::TcpForwardHandler;
use crate::permissions::agent_permission_handler::AgentPermissionHandler;
use crate::shared::commands::realtime_status::RealtimeStatusHandler;
use crate::shared::commands::terminal::RemoteTerminalHandler;
use crate::shared::join_agent::AgentInitPayload;
use crate::util::key_storage::KeySource;
use crate::util::location::Location;
use crate::verifier::upstream_verifier::UpstreamVerifier;

pub struct Agent {
    rpc: RpcClient,
    root_certificate: Certificate,
    credentials: Credential,
    cancel: CancellationToken,
}

impl Agent {
    #[instrument]
    pub async fn open(cancel: CancellationToken) -> Result<Agent> {
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

        let verifier = UpstreamVerifier::new(
            config.root_certificate.clone(),
            config.upstream_certificate.clone(),
        )
        .to_tls_verifier();

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

        Ok(Agent {
            credentials,
            root_certificate: config.root_certificate,
            rpc,
            cancel,
        })
    }

    pub fn certificate(&self) -> &Certificate {
        self.credentials.get_certificate()
    }

    pub async fn run(&self) -> Result<()> {
        let permission_handler = AgentPermissionHandler::new(self.root_certificate.clone());

        let e2e_commands = HandlerCollection::new(permission_handler.clone());

        let updater = Updater::new(self.cancel.clone())
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

        // Todo: proper upstream verifier
        let verifier = ExactVerififier::new(self.root_certificate.clone());

        public_commands.chain().await.add(E2EHandler::new(
            self.credentials.clone(),
            e2e_commands,
            verifier.to_tls_verifier(),
        ));

        let server_commands = HandlerCollection::new(permission_handler);

        server_commands
            .chain()
            .await
            .add(DeauthenticateHandler::new(public_commands));

        self.rpc
            .serve(server_commands)
            .await
            .context("error serving rpc")
    }

    pub async fn init_with(data: AgentInitPayload) -> Result<()> {
        let key_source = KeySource::generate_builtin()?;

        let config = AgentConfig {
            root_certificate: data.root,
            upstream_certificate: data.upstream,
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

    async fn data_dir() -> Result<Location> {
        Location::system_data_dir()?
            .push("agent")
            .ensure_exists()
            .await
    }

    async fn config_path() -> Result<Location> {
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

    async fn save_config(config: &AgentConfig) -> Result<()> {
        let location = Self::config_path().await?;
        let config = serde_json::to_vec_pretty(config)?;
        tokio::fs::write(&location, config).await?;
        Ok(())
    }

    pub async fn close(&self, timeout_duration: Duration) -> Result<(), Elapsed> {
        self.cancel.cancel();
        let result1 = self.rpc.close(timeout_duration).await;

        result1
    }
}

#[derive(Serialize, Deserialize)]
struct AgentConfig {
    upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: Certificate,
    encrypted_credentials: EncryptedCredential,
    key_source: KeySource,
}
