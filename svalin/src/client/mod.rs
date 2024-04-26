use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

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

            let path = PathBuf::try_from(appdata).context("Failed to convert APPDATA to path")?;

            Ok(path)
        }

        #[cfg(target_os = "linux")]
        {}
    }
}
