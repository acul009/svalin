use std::{
    fmt::Display,
    ops::Deref,
    path::{Path, PathBuf},
};

use anyhow::Result;

#[derive(Clone)]
pub struct Location {
    path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum LocationError {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Neither XDG_CONFIG_HOME nor HOME environment variables are set.")]
    NoHomeSet,
    #[error("PROGRAMDATA environment variable is not set.")]
    NoProgramDataSet,
    #[error("APPDATA environment variable is not set.")]
    NoAppDataSet,
}

impl Location {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: PathBuf::from(path.as_ref()),
        }
    }

    pub fn system_data_dir() -> Result<Self, LocationError> {
        #[cfg(test)]
        {
            Ok(Self::new(std::env::current_dir()?).push("test_data"))
        }
        #[cfg(not(test))]
        {
            #[cfg(target_os = "windows")]
            {
                let appdata =
                    std::env::var("PROGRAMDATA").map_err(|_| LocationError::NoProgramDataSet)?;

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

    pub fn user_data_dir() -> Result<Self, LocationError> {
        #[cfg(test)]
        {
            Ok(Self::new(std::env::current_dir()?).push("test_data"))
        }
        #[cfg(not(test))]
        {
            #[cfg(target_os = "windows")]
            {
                use anyhow::Context;

                let appdata = std::env::var("APPDATA").map_err(|_| LocationError::NoAppDataSet)?;

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
                            None => Err(LocationError::NoHomeSet),
                        }
                    }
                }
            }
        }
    }

    pub fn system_temp_dir() -> Result<Self, LocationError> {
        let temp_dir = Self::new(std::env::temp_dir()).push("svalin");
        Ok(temp_dir)
    }

    pub fn push(mut self, path: impl AsRef<Path>) -> Self {
        self.path.push(path);
        self
    }

    pub async fn ensure_parent_exists(self) -> Result<Self, LocationError> {
        let parent = self.parent().unwrap();
        let parent_exists = tokio::fs::try_exists(&parent).await.unwrap_or(false);
        if !parent_exists {
            tokio::fs::create_dir_all(&parent).await?;
        }

        Ok(self)
    }

    pub async fn exists(&self) -> bool {
        tokio::fs::try_exists(&self.path).await.unwrap_or(false)
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
