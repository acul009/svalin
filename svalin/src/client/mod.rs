use std::path::PathBuf;

use anyhow::{Context, Result};

mod first_connect;

pub use first_connect::*;
use svalin_rpc::SkipServerVerification;

pub struct Client;

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
}
