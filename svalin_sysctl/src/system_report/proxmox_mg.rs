use serde::{Deserialize, Serialize};
use tokio::process::Command;

const PMGSH: &str = "/usr/bin/pmgsh";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProxmoxMG {
    pub attachment_quarantine_count: Option<u64>,
    pub virus_quarantine_count: Option<u64>,
}

impl ProxmoxMG {
    pub async fn create() -> anyhow::Result<Option<Self>> {
        let is_proxmox_mg = cfg!(target_os = "linux")
            && tokio::fs::try_exists("/etc/pmg").await?
            && tokio::fs::try_exists("/usr/bin/pmgversion").await?;

        if !is_proxmox_mg {
            return Ok(None);
        }

        if !tokio::fs::try_exists(PMGSH).await? {
            return Ok(Some(Self {
                attachment_quarantine_count: None,
                virus_quarantine_count: None,
            }));
        }

        let endtime = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            .to_string();

        let attachment_args = [
            "get",
            "/quarantine/attachment",
            "--starttime",
            "0",
            "--endtime",
            &endtime,
        ];
        let virus_args = ["get", "/quarantine/virusstatus"];
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

async fn quarantine_count(kind: &str, args: &[&str]) -> Option<u64> {
    let output = match Command::new(PMGSH)
        .args(args)
        .args(["--output-format", "json"])
        .output()
        .await
    {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            tracing::warn!(
                quarantine = kind,
                status = %output.status,
                stderr = %String::from_utf8_lossy(&output.stderr),
                "failed to query Proxmox Mail Gateway quarantine"
            );
            return None;
        }
        Err(error) => {
            tracing::warn!(
                quarantine = kind,
                %error,
                "failed to run Proxmox Mail Gateway CLI"
            );
            return None;
        }
    };

    match parse_quarantine_count(kind, &output.stdout) {
        Some(count) => Some(count),
        None => {
            tracing::warn!(
                quarantine = kind,
                "Proxmox Mail Gateway CLI returned an unexpected response"
            );
            None
        }
    }
}

fn parse_quarantine_count(kind: &str, output: &[u8]) -> Option<u64> {
    let value: serde_json::Value = serde_json::from_slice(output).ok()?;

    match kind {
        "attachment" => u64::try_from(value.as_array()?.len()).ok(),
        "virus" => value.get("count")?.as_u64(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_quarantine_count;

    #[test]
    fn parses_attachment_list_count() {
        assert_eq!(
            parse_quarantine_count("attachment", br#"[{"id":"one"},{"id":"two"}]"#),
            Some(2)
        );
    }

    #[test]
    fn parses_virus_status_count() {
        assert_eq!(
            parse_quarantine_count("virus", br#"{"count":7,"mbytes":1.5}"#),
            Some(7)
        );
    }

    #[test]
    fn rejects_unexpected_output() {
        assert_eq!(parse_quarantine_count("attachment", b"{}"), None);
        assert_eq!(parse_quarantine_count("virus", b"[]"), None);
    }
}
