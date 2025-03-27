use std::{collections::BTreeMap, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Certificate, PermCredentials,
    verifier::{KnownCertificateVerifier, exact::ExactVerififier},
};
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};
use tokio::{sync::watch, task::JoinSet};
use tracing::{debug, error};

use crate::{
    client::tunnel_manager::TunnelManager, shared::commands::agent_list::UpdateAgentList,
    verifier::upstream_verifier::UpstreamVerifier,
};

use super::Client;

#[derive(Serialize, Deserialize)]
pub(crate) struct Profile {
    pub(crate) username: String,
    pub(crate) upstream_address: String,
    pub(crate) upstream_certificate: Certificate,
    pub(crate) root_certificate: Certificate,
    pub(crate) raw_credentials: Vec<u8>,
}

impl Profile {
    pub(crate) fn new(
        username: String,
        upstream_address: String,
        upstream_certificate: Certificate,
        root_certificate: Certificate,
        raw_credentials: Vec<u8>,
    ) -> Self {
        Self {
            username,
            upstream_address,
            upstream_certificate,
            root_certificate,
            raw_credentials,
        }
    }
}

impl Client {
    pub fn get_profiles() -> Result<Vec<String>> {
        let db = Self::open_marmelade()?;

        let profiles = db.list_scopes()?;

        Ok(profiles)
    }

    fn open_marmelade() -> Result<marmelade::DB> {
        let mut path = Self::get_config_dir_path()?;
        path.push("client.jammdb");

        let db = marmelade::DB::open(path)?;
        Ok(db)
    }

    fn get_config_dir_path() -> Result<PathBuf> {
        #[cfg(test)]
        {
            Ok(std::env::current_dir()?)
        }

        #[cfg(not(test))]
        {
            let path = Self::get_general_config_dir_path()?;

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
            let appdata = std::env::var("APPDATA")
                .context("Failed to retrieve APPDATA environment variable")?;

            let path = PathBuf::from(appdata);

            Ok(path)
        }

        #[cfg(target_os = "linux")]
        {
            match std::env::var_os("XDG_CONFIG_HOME") {
                Some(xdg_config_home) => {
                    let mut config_dir = PathBuf::from(xdg_config_home);
                    config_dir.push("svalin");
                    config_dir.push("client");
                    Ok(config_dir)
                }
                None => {
                    // If XDG_CONFIG_HOME is not set, use the default ~/.config directory
                    match std::env::var_os("HOME") {
                        Some(home_dir) => {
                            let mut config_dir = PathBuf::from(home_dir);
                            config_dir.push(".config");
                            config_dir.push("svalin");
                            config_dir.push("client");
                            Ok(config_dir)
                        }
                        None => Err(anyhow!(
                            "Neither XDG_CONFIG_HOME nor HOME environment variables are set."
                        )),
                    }
                }
            }
        }
    }

    pub async fn add_profile(
        username: String,
        upstream_address: String,
        upstream_certificate: Certificate,
        root_certificate: Certificate,
        credentials: PermCredentials,
        password: Vec<u8>,
    ) -> Result<String> {
        let raw_credentials = credentials.to_bytes(password).await?;

        let scope = format!("{username}@{upstream_address}");

        let profile = Profile::new(
            username,
            upstream_address,
            upstream_certificate,
            root_certificate,
            raw_credentials,
        );

        let db = Self::open_marmelade().context("Failed to open marmelade")?;

        db.scope(scope.clone())
            .context("Failed to create profile scope")?
            .update(|b| {
                let current = b.get_kv("profile");
                if current.is_some() {
                    return Err(anyhow!("Profile already exists"));
                }

                b.put_object("profile", &profile)?;

                Ok(())
            })?;

        Ok(scope)
    }

    pub fn remove_profile(profile_key: &str) -> Result<()> {
        let db = Self::open_marmelade()?;
        db.delete_scope(profile_key)?;
        Ok(())
    }

    pub async fn open_profile_string(profile_key: String, password: Vec<u8>) -> Result<Self> {
        Self::open_profile(profile_key, password).await
    }

    pub async fn open_profile(profile_key: String, password: Vec<u8>) -> Result<Self> {
        let db = Self::open_marmelade()?;

        let available_scopes = db.list_scopes()?;

        debug!("Available scopes: {:?}", available_scopes);

        if !available_scopes.contains(&profile_key) {
            return Err(anyhow!("Profile not found"));
        }

        debug!("Opening profile {}", profile_key);

        let mut profile: Option<Profile> = None;

        let scope = db.scope(profile_key)?;

        scope.view(|b| {
            profile = b.get_object("profile")?;

            Ok(())
        })?;

        debug!("Data from profile ready");

        if let Some(profile) = profile {
            debug!("unlocking profile");
            let identity = PermCredentials::from_bytes(&profile.raw_credentials, password).await?;

            debug!("creating verifier");
            let verifier = UpstreamVerifier::new(
                profile.root_certificate.clone(),
                profile.upstream_certificate.clone(),
            )
            .to_tls_verifier();

            debug!("connecting to server");
            let rpc =
                RpcClient::connect(&profile.upstream_address, Some(&identity), verifier).await?;

            debug!("connected to server");

            let tunnel_manager = TunnelManager::new();

            let mut client = Self {
                rpc,
                _upstream_address: profile.upstream_address,
                upstream_certificate: profile.upstream_certificate,
                root_certificate: profile.root_certificate.clone(),
                credentials: identity.clone(),
                device_list: watch::channel(BTreeMap::new()).0,
                tunnel_manager,
                background_tasks: JoinSet::new(),
            };

            let list_clone = client.device_list.clone();
            let sync_connection = client.rpc.upstream_connection();
            let tunnel_manager = client.tunnel_manager.clone();

            client.background_tasks.spawn(async move {
                debug!("subscribing to upstream agent list");
                if let Err(err) = sync_connection
                    .dispatch(UpdateAgentList {
                        base_connection: sync_connection.clone(),
                        credentials: identity,
                        list: list_clone,
                        verifier: ExactVerififier::new(profile.root_certificate),
                        tunnel_manager,
                    })
                    .await
                {
                    error!("error while keeping agent list in sync: {:?}", err);
                }
            });

            Ok(client)
        } else {
            Err(anyhow!("Profile is empty - database is inconsistent"))
        }
    }
}
