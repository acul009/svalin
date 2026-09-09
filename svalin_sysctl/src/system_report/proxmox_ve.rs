use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProxmoxVE {}

impl ProxmoxVE {
    pub async fn create() -> anyhow::Result<Option<Self>> {
        let is_proxmox_ve = cfg!(target_os = "linux")
            && tokio::fs::try_exists("/etc/pve").await?
            && tokio::fs::try_exists("/usr/bin/pveversion").await?;

        if !is_proxmox_ve {
            return Ok(None);
        }

        Ok(Some(Self {}))
    }
}
