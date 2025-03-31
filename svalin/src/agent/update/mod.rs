use std::{
    fmt::{Display, Write},
    io,
    process::ExitStatus,
};

use serde::{Deserialize, Serialize};
use tokio::process;

pub mod check_update;

#[derive(Serialize, Deserialize, Clone)]
pub enum UpdateChannel {
    Alpha,
    Beta,
    Stable,
}

impl Display for UpdateChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateChannel::Alpha => f.write_str("alpha"),
            UpdateChannel::Beta => f.write_str("beta"),
            UpdateChannel::Stable => f.write_str("stable"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UpdateMethod {
    Dpkg,
    None,
}

impl Display for UpdateMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateMethod::Dpkg => f.write_str("dpkg"),
            UpdateMethod::None => f.write_str("-"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateInfo {
    pub current_version: String,
    pub update_method: UpdateMethod,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AvailableUpdateInfo {
    update_type: UpdateMethod,
    available_version: String,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdaterError {
    #[error(transparent)]
    IoError(#[from] io::Error),
}

pub struct Updater;

impl Updater {
    pub async fn get_update_info(&self) -> Result<UpdateInfo, UpdaterError> {
        let update_type = self.get_update_method().await?;

        todo!()
    }

    fn get_current_version(&self) -> &'static str {
        clap::crate_version!()
    }

    async fn get_update_method(&self) -> Result<UpdateMethod, UpdaterError> {
        if self.check_for_dpkg().await? {
            return Ok(UpdateMethod::Dpkg);
        }

        Ok(UpdateMethod::None)
    }

    async fn check_for_dpkg(&self) -> Result<bool, UpdaterError> {
        #[cfg(windows)]
        {
            Ok(false)
        }
        #[cfg(not(windows))]
        {
            let status = process::Command::new("dpkg")
                .arg("--version")
                .status()
                .await?;

            Ok(status.success())
        }
    }
}
