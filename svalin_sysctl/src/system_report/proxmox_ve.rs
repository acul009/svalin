use std::time::Duration;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::process::Command;

pub mod backup;

/// Executes a read-only PVE API query and decodes its JSON response.
async fn query<T: DeserializeOwned>(path: &str, args: &[&str]) -> anyhow::Result<T> {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new("/usr/bin/pvesh")
            .args(["get", path, "--output-format", "json"])
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .context("pvesh timed out")??;
    if !output.status.success() {
        bail!(
            "pvesh get {path}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("invalid pvesh response for {path}"))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProxmoxVE {
    /// None means collection was unavailable; an empty list means no applicable jobs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_jobs: Option<Vec<backup::BackupJob>>,
}

impl ProxmoxVE {
    pub async fn create() -> anyhow::Result<Option<Self>> {
        let is_proxmox_ve = cfg!(target_os = "linux")
            && tokio::fs::try_exists("/etc/pve").await?
            && tokio::fs::try_exists("/usr/bin/pveversion").await?;

        if !is_proxmox_ve {
            return Ok(None);
        }

        Ok(Some(Self {
            backup_jobs: backup::collect().await,
        }))
    }
}
