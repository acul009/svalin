use std::path::PathBuf;

use anyhow::{Context, Result};

mod first_connect;

pub use first_connect::FirstConnect;
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
        {}
    }

    pub async fn first_connect(address: String) -> Result<FirstConnect> {
        let url = url::Url::parse(&format!("svalin://{address}"))?;
        let client = svalin_rpc::Client::connect(url, None, SkipServerVerification::new()).await?;

        todo!()
    }
}