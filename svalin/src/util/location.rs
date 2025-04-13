use std::{
    fmt::Display,
    ops::Deref,
    path::{Path, PathBuf},
};

use anyhow::Result;

pub struct Location {
    path: PathBuf,
}

impl Location {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: PathBuf::from(path.as_ref()),
        }
    }

    pub fn system_data_dir() -> Result<Self> {
        #[cfg(test)]
        {
            Ok(Self::new(std::env::current_dir()?).push("test_data"))
        }
        #[cfg(not(test))]
        {
            #[cfg(target_os = "windows")]
            {
                use anyhow::Context;

                let appdata = std::env::var("PROGRAMDATA")
                    .context("Failed to retrieve PROGRAMMDATA environment variable")?;

                let mut path = PathBuf::from(appdata);

                path.push("svalin");

                Ok(Self::new(path))
            }

            #[cfg(target_os = "linux")]
            {
                Ok(Self::new("/var/lib/svalin/agent"))
            }
        }
    }

    pub fn user_data_dir() -> Result<Self> {
        #[cfg(test)]
        {
            Ok(Self::new(std::env::current_dir()?).push("test_data"))
        }
        #[cfg(not(test))]
        {
            #[cfg(target_os = "windows")]
            {
                let appdata = std::env::var("APPDATA")
                    .context("Failed to retrieve APPDATA environment variable")?;

                let path = PathBuf::from(appdata);

                Ok(Self::new(path).push("svalin"))
            }

            #[cfg(target_os = "linux")]
            {
                match std::env::var_os("XDG_CONFIG_HOME") {
                    Some(xdg_config_home) => {
                        let config_dir = PathBuf::from(xdg_config_home);
                        Ok(Self::new(config_dir).push("svalin"))
                    }
                    None => {
                        // If XDG_CONFIG_HOME is not set, use the default ~/.config directory
                        match std::env::var_os("HOME") {
                            Some(home_dir) => {
                                let config_dir = PathBuf::from(home_dir);
                                Ok(Self::new(config_dir).push(".config").push("svalin"))
                            }
                            None => Err(anyhow::anyhow!(
                                "Neither XDG_CONFIG_HOME nor HOME environment variables are set."
                            )),
                        }
                    }
                }
            }
        }
    }

    pub fn push(mut self, path: impl AsRef<Path>) -> Self {
        self.path.push(path);
        self
    }

    pub async fn ensure_exists(self) -> Result<Self> {
        if !self.path.exists() {
            tokio::fs::create_dir_all(&self.path).await?;
        }

        Ok(self)
    }

    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

impl Deref for Location {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.as_path()
    }
}

impl AsRef<Path> for Location {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}
