use std::{fmt::Display, io, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::{
    fs::{self},
    io::AsyncWriteExt,
    process::Command,
    sync::watch,
};
use tokio_util::sync::CancellationToken;
use tracing::debug;

pub mod request_available_version;
pub mod request_installation_info;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

impl UpdateChannel {
    fn from_version(version: &str) -> Self {
        if version.ends_with("-alpha") {
            Self::Alpha
        } else if version.ends_with("-beta") {
            Self::Beta
        } else {
            Self::Stable
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum InstallMethod {
    Dpkg,
    Unknown,
}

impl Display for InstallMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallMethod::Dpkg => f.write_str("dpkg"),
            InstallMethod::Unknown => f.write_str("?"),
        }
    }
}

impl InstallMethod {
    pub fn supports_update(&self) -> bool {
        match &self {
            InstallMethod::Dpkg => true,
            InstallMethod::Unknown => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstallationInfo {
    pub install_method: InstallMethod,
    pub current_version: String,
    pub currently_updating: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdaterError {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error("already updating, lock error: {0}")]
    AlreadyUpdating(#[from] tokio::sync::TryLockError),
    #[error("unable to install update: unsupported install method")]
    UnsupportedInstallMethod,
    #[error("error in crate reqwest: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("There is no version available for this channel")]
    NoVersionForChannel,
    #[error("The selected release {0} seems to be missing the correct file type")]
    MissingFileInRelease(String),
    #[error("error while running dpkg")]
    DpkgError,
}

pub struct Updater {
    watch: watch::Sender<InstallationInfo>,
    update_lock: tokio::sync::Mutex<()>,
    /// This token has to shut down the agent in order to deploy the update.
    /// The agent is meant to be run as a service, so it's started back up again
    /// with the new version.
    shutdown_token: CancellationToken,
}

const GITHUB_REPO: &str = "acul009/svalin";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

impl Updater {
    pub async fn new(shutdown_token: CancellationToken) -> Result<Arc<Self>, UpdaterError> {
        let install_info = Self::get_install_info().await?;
        Ok(Arc::new(Self {
            watch: watch::Sender::new(install_info),
            update_lock: tokio::sync::Mutex::new(()),
            shutdown_token,
        }))
    }

    pub fn subscribe_install_info(&self) -> watch::Receiver<InstallationInfo> {
        self.watch.subscribe()
    }

    async fn get_install_info() -> Result<InstallationInfo, UpdaterError> {
        let install_method = Self::get_install_method().await?;

        Ok(InstallationInfo {
            current_version: Self::get_current_version().to_string(),
            install_method,
            currently_updating: false,
        })
    }

    fn get_current_version() -> String {
        format!("v{}", clap::crate_version!())
    }

    async fn get_install_method() -> Result<InstallMethod, UpdaterError> {
        if Self::check_for_dpkg().await? {
            return Ok(InstallMethod::Dpkg);
        }

        Ok(InstallMethod::Unknown)
    }

    async fn check_for_dpkg() -> Result<bool, UpdaterError> {
        #[cfg(windows)]
        {
            Ok(false)
        }
        #[cfg(not(windows))]
        {
            let status = tokio::process::Command::new("dpkg")
                .arg("--version")
                .status()
                .await?;

            // Todo: check if it was actually installed via dpkg

            Ok(status.success())
        }
    }

    pub async fn check_channel_version(
        &self,
        channel: &UpdateChannel,
    ) -> Result<String, UpdaterError> {
        let install_method = self.watch.borrow().install_method.clone();

        if !install_method.supports_update() {
            return Err(UpdaterError::UnsupportedInstallMethod);
        }

        debug!("install type supports updating");

        match install_method {
            InstallMethod::Dpkg => Self::get_github_channel_version(channel).await,
            InstallMethod::Unknown => unreachable!(),
        }
    }

    async fn get_github_channel_version(channel: &UpdateChannel) -> Result<String, UpdaterError> {
        match Self::get_github_release_for_channel(channel).await? {
            Some(release) => Ok(release.name),
            None => Err(UpdaterError::NoVersionForChannel),
        }
    }

    async fn get_github_release_for_channel(
        channel: &UpdateChannel,
    ) -> Result<Option<GithubRelease>, UpdaterError> {
        Ok(Self::get_github_releases()
            .await?
            .into_iter()
            .filter(|release| &UpdateChannel::from_version(&release.name) == channel)
            .next())
    }

    async fn get_github_releases() -> Result<Vec<GithubRelease>, UpdaterError> {
        let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases");

        // Get the newest releases
        let releases: Vec<GithubRelease> = reqwest::Client::new()
            .get(url)
            .header("User-Agent", "Svalin")
            .send()
            .await?
            .json()
            .await?;

        // debug!("releases: {:?}", releases);

        Ok(releases)
    }

    pub async fn update_to(&self, channel: &UpdateChannel) -> Result<(), UpdaterError> {
        let _lock = self.update_lock.try_lock()?;
        let mut install_info = self.watch.borrow().clone();
        let install_method = install_info.install_method.clone();

        if !install_info.install_method.supports_update() {
            return Err(UpdaterError::UnsupportedInstallMethod);
        }

        install_info.currently_updating = true;
        let _ = self.watch.send(install_info);

        let update_result = match install_method {
            InstallMethod::Dpkg => self.update_dpkg(channel).await,
            InstallMethod::Unknown => unreachable!(),
        };

        if update_result.is_ok() {
            // send signal to shut down agent
            self.shutdown_token.cancel();
        }

        update_result
    }

    pub async fn update_dpkg(&self, channel: &UpdateChannel) -> Result<(), UpdaterError> {
        let release = match Self::get_github_release_for_channel(channel).await? {
            None => return Err(UpdaterError::NoVersionForChannel),
            Some(release) => release,
        };

        let download_url = match release
            .assets
            .into_iter()
            .filter(|asset| asset.name.ends_with(".deb"))
            .next()
        {
            None => return Err(UpdaterError::MissingFileInRelease(release.name)),
            Some(asset) => asset.browser_download_url,
        };

        fs::create_dir("/tmp/svalin_update").await?;

        let mut request = reqwest::get(download_url).await?;
        let mut deb_file = fs::File::create("/tmp/svalin_update/svalin.deb").await?;

        while let Some(chunk) = request.chunk().await? {
            deb_file.write_all(&chunk).await?;
        }

        let install_result = Command::new("dpkg")
            .arg("-i")
            .arg("/tmp/svalin_update/svalin.deb")
            .status()
            .await?;

        if install_result.success() {
            Ok(())
        } else {
            Err(UpdaterError::DpkgError)
        }
    }
}
