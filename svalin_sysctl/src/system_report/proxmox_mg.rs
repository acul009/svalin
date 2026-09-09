use serde::{Deserialize, Serialize};
use tokio::process::Command;

const PMGSH: &str = "/usr/bin/pmgsh";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProxmoxMG {
    pub attachment_quarantine_count: u64,
    pub virus_quarantine_count: u64,
}

impl ProxmoxMG {
    pub async fn create() -> anyhow::Result<Option<Self>> {
        let is_proxmox_mg = cfg!(target_os = "linux")
            && tokio::fs::try_exists("/etc/pmg").await?
            && tokio::fs::try_exists("/usr/bin/pmgversion").await?;

        if !is_proxmox_mg {
            return Ok(None);
        }

        let endtime = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let starttime = endtime.saturating_sub(7 * 24 * 60 * 60).to_string();
        let endtime = endtime.to_string();

        let attachment_args = [
            "get",
            "/quarantine/attachment",
            "--starttime",
            &starttime,
            "--endtime",
            &endtime,
        ];
        let virus_args = ["get", "/quarantine/virus"];
        let (attachment_quarantine_count, virus_quarantine_count) = tokio::join!(
            quarantine_count("attachment", &attachment_args),
            quarantine_count("virus", &virus_args),
        );

        Ok(Some(Self {
            attachment_quarantine_count,
            virus_quarantine_count,
        }))
    }
}

async fn quarantine_count(kind: &str, args: &[&str]) -> u64 {
    let output = match Command::new(PMGSH).args(args).output().await {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            tracing::warn!(
                quarantine = kind,
                status = %output.status,
                stderr = %String::from_utf8_lossy(&output.stderr),
                "failed to query Proxmox Mail Gateway quarantine"
            );
            return 0;
        }
        Err(error) => {
            tracing::warn!(
                quarantine = kind,
                %error,
                "failed to run Proxmox Mail Gateway CLI"
            );
            return 0;
        }
    };

    parse_quarantine_count(&output.stdout)
}

fn parse_quarantine_count(output: &[u8]) -> u64 {
    let utf = String::from_utf8_lossy(output);
    utf.lines()
        .filter(|line| line.trim_ascii_start().starts_with("\"bytes\""))
        .count() as u64
}
