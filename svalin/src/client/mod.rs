use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};

mod first_connect;
pub mod verifiers;

mod profile;
pub use profile::*;

pub use first_connect::*;
use svalin_pki::{Certificate, PermCredentials};

pub struct Client {
    rpc: svalin_rpc::Client,
}

impl Client {
    pub fn get_profiles() -> Result<Vec<String>> {
        let db = Self::open_marmelade()?;

        let profiles = db.list_scopes()?;

        Ok(profiles)
    }

    fn open_marmelade() -> Result<marmelade::DB> {
        let mut path = Self::get_config_dir_path()?;
        path.push("marmelade.jammdb");

        Ok(marmelade::DB::open(path)?)
    }

    fn get_config_dir_path() -> Result<PathBuf> {
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

    pub fn add_profile(
        username: String,
        upstream_address: String,
        upstream_certificate: Certificate,
        root_certificate: Certificate,
        credentials: PermCredentials,
        password: &[u8],
    ) -> Result<()> {
        let raw_credentials = credentials.to_bytes(password)?;

        let scope = format!("{username}@{upstream_address}");

        let profile = Profile::new(
            username,
            upstream_address,
            upstream_certificate,
            root_certificate,
            raw_credentials,
        );

        let db = Self::open_marmelade()?;

        db.scope(scope)
            .context("Failed to create profile scope")?
            .update(|b| {
                let current = b.get_kv("profile");
                if current.is_some() {
                    return Err(anyhow!("Profile already exists"));
                }

                b.put_object("profile", &profile)?;

                Ok(())
            })?;

        Ok(())
    }

    pub fn remove_profile(profile_key: &str) -> Result<()> {
        let db = Self::open_marmelade()?;
        db.delete_scope(profile_key)?;
        Ok(())
    }

    pub async fn open_profile(profile_key: String, password: &[u8]) -> Result<Self> {
        let db = Self::open_marmelade()?;

        if !db.list_scopes()?.contains(&profile_key) {
            return Err(anyhow!("Profile not found"));
        }

        let mut profile: Option<Profile> = None;

        let scope = db.scope(profile_key)?;

        scope.view(|b| {
            profile = b.get_object("profile")?;

            Ok(())
        })?;

        if let Some(profile) = profile {
            let identity = PermCredentials::from_bytes(&profile.raw_credentials, password)?;

            let verifier = verifiers::upstream_verifier::UpstreamVerifier::new(
                profile.root_certificate,
                profile.upstream_certificate,
            );

            let rpc =
                svalin_rpc::Client::connect(profile.upstream_address, Some(&identity), verifier)
                    .await?;

            Ok(Self { rpc })
        } else {
            Err(anyhow!("Profile is empty - database is inconsistent"))
        }
    }

    pub fn rpc(&self) -> &svalin_rpc::Client {
        &self.rpc
    }

    pub fn close(&self) {
        self.rpc.close()
    }
}
