use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::process::Command;

const BITLOCKER_COMMAND: &str = r#"Get-BitLockerVolume | ForEach-Object {
    $passwords = [string[]]@(
        $_.KeyProtector |
            Where-Object KeyProtectorType -eq 'RecoveryPassword' |
            ForEach-Object RecoveryPassword
    )

    [pscustomobject]@{
        MountPoint           = $_.MountPoint
        VolumeStatus         = $_.VolumeStatus
        ProtectionStatus     = $_.ProtectionStatus
        EncryptionPercentage = $_.EncryptionPercentage
        RecoveryPasswords    = $passwords
    }
} | ConvertTo-Json -Depth 3"#;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BitLockerVolume {
    pub mount_point: String,
    pub volume_status: VolumeStatus,
    pub protection_status: ProtectionStatus,
    pub encryption_percentage: u8,
    pub recovery_passwords: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeStatus {
    FullyDecrypted,
    FullyEncrypted,
    EncryptionInProgress,
    DecryptionInProgress,
    EncryptionPaused,
    DecryptionPaused,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtectionStatus {
    Off,
    On,
    Unknown,
}

pub(super) async fn query_volumes() -> Option<Vec<BitLockerVolume>> {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                BITLOCKER_COMMAND,
            ])
            .kill_on_drop(true)
            .output(),
    )
    .await;

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!(%error, "BitLocker query timed out");
            return None;
        }
    };

    match output {
        Ok(output) if output.status.success() => match parse_bitlocker_volumes(&output.stdout) {
            Ok(volumes) => Some(volumes),
            Err(error) => {
                tracing::warn!(%error, "BitLocker returned an unexpected response");
                None
            }
        },
        Ok(output) => {
            tracing::warn!(
                status = %output.status,
                stderr = %String::from_utf8_lossy(&output.stderr),
                "failed to query BitLocker volumes"
            );
            None
        }
        Err(error) => {
            tracing::warn!(%error, "failed to run the BitLocker PowerShell query");
            None
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawBitLockerVolume {
    mount_point: String,
    volume_status: u8,
    protection_status: u8,
    encryption_percentage: u8,
    recovery_passwords: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawBitLockerOutput {
    Multiple(Vec<RawBitLockerVolume>),
    Single(RawBitLockerVolume),
}

fn parse_bitlocker_volumes(output: &[u8]) -> anyhow::Result<Vec<BitLockerVolume>> {
    let raw = match serde_json::from_slice(output)? {
        RawBitLockerOutput::Multiple(volumes) => volumes,
        RawBitLockerOutput::Single(volume) => vec![volume],
    };

    raw.into_iter()
        .map(|volume| {
            Ok(BitLockerVolume {
                mount_point: volume.mount_point,
                volume_status: match volume.volume_status {
                    0 => VolumeStatus::FullyDecrypted,
                    1 => VolumeStatus::FullyEncrypted,
                    2 => VolumeStatus::EncryptionInProgress,
                    3 => VolumeStatus::DecryptionInProgress,
                    4 => VolumeStatus::EncryptionPaused,
                    5 => VolumeStatus::DecryptionPaused,
                    value => anyhow::bail!("unknown BitLocker volume status {value}"),
                },
                protection_status: match volume.protection_status {
                    0 => ProtectionStatus::Off,
                    1 => ProtectionStatus::On,
                    2 => ProtectionStatus::Unknown,
                    value => anyhow::bail!("unknown BitLocker protection status {value}"),
                },
                encryption_percentage: volume.encryption_percentage,
                recovery_passwords: volume.recovery_passwords,
            })
        })
        .collect()
}
