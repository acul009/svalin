use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProxmoxBS {}

impl ProxmoxBS {
    pub async fn create() -> anyhow::Result<Option<Self>> {
        let is_proxmox_bs = cfg!(target_os = "linux")
            && tokio::fs::try_exists("/etc/proxmox-backup").await?
            && tokio::fs::try_exists("/usr/bin/proxmox-backup-manager").await?;

        if !is_proxmox_bs {
            return Ok(None);
        }

        Ok(Some(Self {}))
    }
}
