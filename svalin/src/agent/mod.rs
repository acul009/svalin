use std::path::PathBuf;
use std::thread::scope;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, PermCredentials};
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::skip_verify::SkipServerVerification;
use tracing::debug;

mod init;

use crate::shared::commands::public_server_status::get_public_statusDispatcher;
use crate::shared::join_agent::AgentInitPayload;

pub struct Agent {
    rpc: RpcClient,
    upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: Certificate,
    credentials: PermCredentials,
}

impl Agent {
    pub async fn init_cmd(address: String) -> Result<()> {
        println!("===============================\nWelcome to svalin!\n===============================\nInitializing Agent...");

        debug!("try connecting to {address}");

        let client = RpcClient::connect(&address, None, SkipServerVerification::new()).await?;

        debug!("successfully connected");

        let mut conn = client.upstream_connection();

        debug!("requesting public status");

        let server_status = conn.get_public_status().await?;

        debug!("public status: {server_status:?}");

        match server_status {
            crate::shared::commands::public_server_status::PublicStatus::WaitingForInit => todo!(),
            crate::shared::commands::public_server_status::PublicStatus::Ready => todo!(),
        }
        todo!()
    }

    pub async fn init_with(data: AgentInitPayload) -> Result<()> {
        let db = Self::open_marmelade()?;

        let scope = db.scope("default".into())?;

        let config = AgentConfig {
            root_certificate: data.root,
            upstream_certificate: data.upstream,
            encrypted_credentials: Self::encrypt_credentials(data.credentials)?,
            upstream_address: data.address,
        };

        scope.update(|b| {
            let current = b.get_kv("p");

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

    fn encrypt_credentials(credentials: PermCredentials) -> Result<Vec<u8>> {
        todo!()
    }

    fn decrypt_credentials(encrypted_credentials: Vec<u8>) -> Result<PermCredentials> {
        todo!()
    }
}

#[derive(Serialize, Deserialize)]
struct AgentConfig {
    upstream_address: String,
    upstream_certificate: Certificate,
    root_certificate: Certificate,
    encrypted_credentials: Vec<u8>,
}
