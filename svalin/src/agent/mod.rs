use std::path::PathBuf;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use svalin_pki::verifier::KnownCertificateVerifier;
use svalin_pki::verifier::exact::ExactVerififier;
use svalin_pki::{Certificate, PermCredentials};
use svalin_rpc::commands::deauthenticate::DeauthenticateHandler;
use svalin_rpc::commands::e2e::E2EHandler;
use svalin_rpc::commands::ping::PingHandler;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::rpc::command::handler::HandlerCollection;
use tracing::{debug, instrument};

mod init;

use crate::client::tunnel_manager::tcp::handler::TcpForwardHandler;
use crate::permissions::agent_permission_handler::AgentPermissionHandler;
use crate::shared::commands::realtime_status::RealtimeStatusHandler;
use crate::shared::commands::terminal::RemoteTerminalHandler;
use crate::shared::join_agent::AgentInitPayload;
use crate::verifier::upstream_verifier::UpstreamVerifier;

pub struct Agent {
    rpc: RpcClient,
    upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: Certificate,
    credentials: PermCredentials,
}

const BASE_CONFIG_KEY: &[u8] = b"base_config";

impl Agent {
    #[instrument]
    pub async fn open() -> Result<Agent> {
        debug!("opening agent configuration");

        let tree = Self::open_db()?;

        let raw_config = tree
            .get(BASE_CONFIG_KEY)?
            .ok_or_else(|| anyhow!("agent not yet configured"))?;

        let config: AgentConfig = postcard::from_bytes(&raw_config)?;

        debug!("decrypting agent credentials");

        let credentials =
            Self::decrypt_credentials(config.encrypted_credentials, tree.clone()).await?;

        debug!("building upstream verifier");

        let verifier = UpstreamVerifier::new(
            config.root_certificate.clone(),
            config.upstream_certificate.clone(),
        )
        .to_tls_verifier();

        debug!("trying to connect to server");

        let rpc =
            RpcClient::connect(&config.upstream_address, Some(&credentials), verifier).await?;

        debug!("connection to server established");

        Ok(Agent {
            credentials: credentials,
            root_certificate: config.root_certificate,
            rpc: rpc,
            upstream_address: config.upstream_address,
            upstream_certificate: config.upstream_certificate,
        })
    }

    pub fn certificate(&self) -> &Certificate {
        self.credentials.get_certificate()
    }

    pub async fn run(&self) -> Result<()> {
        let permission_handler = AgentPermissionHandler::new(self.root_certificate.clone());

        let e2e_commands = HandlerCollection::new(permission_handler.clone());

        e2e_commands
            .chain()
            .await
            .add(PingHandler)
            .add(RealtimeStatusHandler)
            .add(RemoteTerminalHandler)
            .add(TcpForwardHandler);

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

        self.rpc.serve(server_commands).await
    }

    pub async fn init_with(data: AgentInitPayload) -> Result<()> {
        let tree = Self::open_db()?;

        let config = AgentConfig {
            root_certificate: data.root,
            upstream_certificate: data.upstream,
            encrypted_credentials: Self::encrypt_credentials(data.credentials, tree.clone())
                .await?,
            upstream_address: data.address,
        };

        if tree.get(BASE_CONFIG_KEY)?.is_some() {
            return Err(anyhow!("Profile already exists"));
        }

        tree.insert(BASE_CONFIG_KEY, postcard::to_extend(&config, Vec::new())?)?;

        tree.flush_async().await?;

        Ok(())
    }

    fn open_db() -> Result<sled::Tree> {
        let mut path = Self::get_config_dir_path()?;
        path.push("client.sled");

        Ok(sled::open(path)?.open_tree("default")?)
    }

    fn get_config_dir_path() -> Result<PathBuf> {
        #[cfg(test)]
        {
            Ok(std::env::current_dir()?)
        }

        #[cfg(not(test))]
        {
            let mut path = Self::get_general_config_dir_path()?;

            path.push("agent");

            // check if config dir exists
            if !path.exists() {
                std::fs::create_dir_all(&path)?;
            }

            Ok(path)
        }
    }

    #[cfg(not(test))]
    fn get_general_config_dir_path() -> Result<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            use anyhow::Context;

            let appdata = std::env::var("PROGRAMDATA")
                .context("Failed to retrieve PROGRAMMDATA environment variable")?;

            let mut path = PathBuf::from(appdata);

            path.push("svalin");

            Ok(path)
        }

        #[cfg(target_os = "linux")]
        {
            Ok(PathBuf::from("/var/lib/svalin/agent"))
        }
    }

    async fn encrypt_credentials(
        credentials: PermCredentials,
        tree: sled::Tree,
    ) -> Result<Vec<u8>> {
        let db_key = "credential_key";

        let key_source = tree
            .get(db_key)?
            .map(|key_source| postcard::from_bytes::<KeySource>(&key_source));

        let key_source = match key_source {
            None => {
                let key_source = KeySource::BuiltIn(svalin_pki::generate_key()?.to_vec());

                tree.insert(db_key, postcard::to_extend(&key_source, Vec::new())?)?;

                key_source
            }
            Some(key_source) => key_source?,
        };

        let key = Self::source_to_key(key_source).await?;

        credentials.to_bytes(key).await
    }

    async fn decrypt_credentials(
        encrypted_credentials: Vec<u8>,
        tree: sled::Tree,
    ) -> Result<PermCredentials> {
        let key_source = tree
            .get("credential_key")?
            .ok_or(anyhow!("no keysource saved in db"))?;

        let key_source = postcard::from_bytes(&key_source)?;

        let key = Self::source_to_key(key_source).await?;
        debug!("Agent password loaded, decrypting...");
        Ok(PermCredentials::from_bytes(&encrypted_credentials, key).await?)
    }

    async fn source_to_key(source: KeySource) -> Result<Vec<u8>> {
        match source {
            KeySource::BuiltIn(k) => Ok(k),
        }
    }

    pub fn close(self) {
        self.rpc.close()
    }
}

#[derive(Serialize, Deserialize)]
struct AgentConfig {
    upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: Certificate,
    encrypted_credentials: Vec<u8>,
}

/// The keysource enum is saved in the agent configuration and specifies how to
/// load the key for decrypting the credentials This will enable the use of
/// external key management systems should that be necessary one day
#[derive(Serialize, Deserialize)]
enum KeySource {
    BuiltIn(Vec<u8>),
}
