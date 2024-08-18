use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use marmelade::Scope;
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, PermCredentials};
use svalin_rpc::commands::forward::E2EHandler;
use svalin_rpc::commands::ping::PingHandler;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::rpc::command::HandlerCollection;
use tracing::{debug, instrument};

mod init;

use crate::client::verifiers;
use crate::shared::commands::realtime_status::RealtimeStatusHandler;
use crate::shared::join_agent::AgentInitPayload;

pub struct Agent {
    rpc: RpcClient,
    upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: Certificate,
    credentials: PermCredentials,
}

impl Agent {
    #[instrument]
    pub async fn open() -> Result<Agent> {
        debug!("opening agent configuration");

        let db = Self::open_marmelade()?;

        let scope = db.scope("default".into())?;

        let config: AgentConfig = scope
            .get_object("base_config")?
            .ok_or(anyhow!("agent not yet configured"))?;

        debug!("decrypting credentials");

        let credentials =
            Self::decrypt_credentials(config.encrypted_credentials, scope.clone()).await?;

        let verifier = verifiers::upstream_verifier::UpstreamVerifier::new(
            config.root_certificate.clone(),
            config.upstream_certificate.clone(),
        );

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
        let e2e_commands = HandlerCollection::new();

        e2e_commands
            .chain()
            .await
            .add(PingHandler::new())
            .add(RealtimeStatusHandler::new());

        let public_commands = HandlerCollection::new();

        public_commands
            .chain()
            .await
            .add(E2EHandler::new(self.credentials.clone(), e2e_commands));

        self.rpc.serve(public_commands).await
    }

    pub async fn init_with(data: AgentInitPayload) -> Result<()> {
        let db = Self::open_marmelade()?;

        let scope = db.scope("default".into())?;

        let config = AgentConfig {
            root_certificate: data.root,
            upstream_certificate: data.upstream,
            encrypted_credentials: Self::encrypt_credentials(data.credentials, scope.clone())
                .await?,
            upstream_address: data.address,
        };

        scope.update(|b| {
            let current = b.get_kv("base_config");

            if current.is_some() {
                return Err(anyhow!("Profile already exists"));
            }

            b.put_object("base_config", &config)?;

            Ok(())
        })?;

        Ok(())
    }

    fn open_marmelade() -> Result<marmelade::DB> {
        let mut path = Self::get_config_dir_path()?;
        path.push("client.jammdb");

        Ok(marmelade::DB::open(path)?)
    }

    fn get_config_dir_path() -> Result<PathBuf> {
        #[cfg(test)]
        {
            Ok(std::env::current_dir()?)
        }

        #[cfg(not(test))]
        {
            let mut path = Self::get_general_config_dir_path()?;

            // check if config dir exists
            if !path.exists() {
                std::fs::create_dir_all(&path)?;
            }

            Ok(path)
        }
    }

    fn get_general_config_dir_path() -> Result<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("PROGRAMDATA")
                .context("Failed to retrieve PROGRAMMDATA environment variable")?;

            let path = PathBuf::from(appdata);

            Ok(path)
        }

        #[cfg(target_os = "linux")]
        {
            Ok(PathBuf::from("/etc/svalin/agent"))
        }
    }

    async fn encrypt_credentials(credentials: PermCredentials, scope: Scope) -> Result<Vec<u8>> {
        let key_source: Option<KeySource> = scope.get_object("credential_key")?;

        let key_source = match key_source {
            Some(k) => k,
            None => {
                let key = KeySource::BuiltIn(Vec::from(svalin_pki::generate_key()?));

                scope.put_object("credential_key".into(), &key)?;

                key
            }
        };

        let key = Self::source_to_key(key_source).await?;

        credentials.to_bytes(key).await
    }

    async fn decrypt_credentials(
        encrypted_credentials: Vec<u8>,
        scope: Scope,
    ) -> Result<PermCredentials> {
        let key_source: Option<KeySource> = scope.get_object("credential_key")?;

        if let Some(key_source) = key_source {
            let key = Self::source_to_key(key_source).await?;

            PermCredentials::from_bytes(&encrypted_credentials, key).await
        } else {
            return Err(anyhow!("no keysource saved in DB"));
        }
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

#[derive(Serialize, Deserialize)]
enum KeySource {
    BuiltIn(Vec<u8>),
}
