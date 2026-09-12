use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};
use tokio::process::Command;

use crate::util::string_enum;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Zfs {
    pub userspace_version: Option<String>,
    pub kernel_version: Option<String>,
    pub pools: BTreeMap<String, ZfsPool>,
}

impl Zfs {
    /// Collects imported pools. Missing tooling or failed collection is logged
    /// and omitted; a successful empty pool list also omits the extension.
    pub async fn create() -> Option<Self> {
        match Self::collect().await {
            Ok(report) => report,
            Err(error) => {
                tracing::warn!(%error, "failed to collect ZFS pools");
                None
            }
        }
    }

    async fn collect() -> anyhow::Result<Option<Self>> {
        let Some(output) = query(&["status", "-j", "--json-int", "-p", "-P"]).await? else {
            return Ok(None);
        };
        let status: PoolOutput = serde_json::from_slice(&output)?;
        if status.pools.is_empty() {
            return Ok(None);
        }
        let mut report = Self {
            userspace_version: None,
            kernel_version: None,
            pools: status.pools,
        };
        match query_version().await {
            Ok(version) => {
                report.userspace_version = version.userland;
                report.kernel_version = version.kernel;
            }
            Err(error) => tracing::warn!(%error, "failed to collect ZFS version"),
        }
        Ok(Some(report))
    }
}

async fn query_version() -> anyhow::Result<VersionFields> {
    let output = query(&["version", "-j"])
        .await?
        .ok_or_else(|| anyhow::anyhow!("zpool unavailable for version query"))?;
    Ok(serde_json::from_slice::<VersionOutput>(&output)?.zfs_version)
}

// Only the command envelopes are separate: pool and device data deserialize
// directly into the public report types.
#[derive(Deserialize)]
struct PoolOutput {
    pools: BTreeMap<String, ZfsPool>,
}

#[derive(Deserialize)]
struct VersionOutput {
    zfs_version: VersionFields,
}

#[derive(Deserialize)]
struct VersionFields {
    userland: Option<String>,
    kernel: Option<String>,
}

async fn query(args: &[&str]) -> anyhow::Result<Option<Vec<u8>>> {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new("zpool")
            .args(args)
            .env("LC_ALL", "C")
            .kill_on_drop(true)
            .output(),
    )
    .await?;
    let output = match output {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        output => output?,
    };
    anyhow::ensure!(
        output.status.success(),
        "zpool {} failed ({}): {}",
        args[0],
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(Some(output.stdout))
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ZfsPool {
    #[serde(rename = "pool_guid", alias = "guid")]
    pub guid: u64,
    pub name: String,
    #[serde(rename = "state")]
    pub health: ZfsHealth,
    pub status: Option<String>,
    pub action: Option<String>,
    /// Known data errors, distinct from device I/O error counters.
    #[serde(rename = "error_count")]
    pub data_errors: Option<u64>,
    #[serde(default)]
    pub vdevs: BTreeMap<String, ZfsVdev>,
    #[serde(rename = "scan_stats")]
    pub scan: Option<ZfsScan>,
}

impl ZfsPool {
    /// Critical: degraded/failed pool or device, or known data corruption.
    /// Device roles do not exempt failed caches or spares.
    pub fn is_problematic(&self) -> bool {
        self.health.is_problematic()
            || self.data_errors.is_some_and(|errors| errors > 0)
            || self.vdevs.values().any(ZfsVdev::is_problematic)
    }

    /// Includes critical problems, advisory messages, recorded I/O errors,
    /// unknown states, and scans requiring attention.
    pub fn needs_attention(&self) -> bool {
        self.is_problematic()
            || self.health != ZfsHealth::Online
            || self.status.as_ref().is_some_and(|status| {
                !status.trim().is_empty() && !status.trim().eq_ignore_ascii_case("OK")
            })
            || self.vdevs.values().any(ZfsVdev::needs_attention)
            || self.scan.as_ref().is_some_and(ZfsScan::needs_attention)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ZfsVdev {
    pub guid: Option<u64>,
    pub name: String,
    pub path: Option<String>,
    #[serde(rename = "vdev_type")]
    pub kind: ZfsVdevKind,
    pub class: Option<ZfsVdevClass>,
    pub state: ZfsHealth,
    #[serde(flatten)]
    pub errors: ZfsIoErrors,
    #[serde(default, rename = "vdevs")]
    pub children: BTreeMap<String, ZfsVdev>,
}

impl ZfsVdev {
    fn is_problematic(&self) -> bool {
        self.state.is_problematic() || self.children.values().any(Self::is_problematic)
    }

    fn needs_attention(&self) -> bool {
        // Available/in-use spares are normal, but unknown states remain advisory.
        let healthy_spare = self.class == Some(ZfsVdevClass::Spare)
            && matches!(self.state, ZfsHealth::Available | ZfsHealth::InUse);
        (self.state != ZfsHealth::Online && !healthy_spare)
            || self.errors.read > 0
            || self.errors.write > 0
            || self.errors.checksum > 0
            || self.children.values().any(Self::needs_attention)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ZfsIoErrors {
    #[serde(rename = "read_errors")]
    pub read: u64,
    #[serde(rename = "write_errors")]
    pub write: u64,
    #[serde(rename = "checksum_errors")]
    pub checksum: u64,
}

/// The reported scan snapshot, not a history of successful scrubs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ZfsScan {
    #[serde(rename = "function")]
    pub kind: ZfsScanKind,
    pub state: ZfsScanState,
    /// Unix timestamp in seconds; zero means not set.
    #[serde(rename = "start_time")]
    pub started_at: u64,
    /// Unix timestamp in seconds; zero means not set.
    #[serde(rename = "end_time")]
    pub finished_at: u64,
    /// Unix timestamp in seconds; zero means not set.
    #[serde(rename = "scrub_pause")]
    pub paused_at: u64,
    pub errors: u64,
}

impl ZfsScan {
    fn needs_attention(&self) -> bool {
        self.errors > 0
            || self.paused_at != 0
            || matches!(self.kind, ZfsScanKind::Other(_))
            || matches!(self.state, ZfsScanState::Canceled | ZfsScanState::Other(_))
            || (self.kind == ZfsScanKind::Resilver && self.state == ZfsScanState::Scanning)
    }
}

string_enum!(ZfsHealth {
    Available => "AVAIL",
    InUse => "INUSE",
    Online => "ONLINE" | "HEALTHY",
    Degraded => "DEGRADED",
    Faulted => "FAULTED",
    Offline => "OFFLINE",
    Removed => "REMOVED",
    Unavailable => "UNAVAIL" | "CANT_OPEN",
    Suspended => "SUSPENDED",
    Unknown => "UNKNOWN",
});

impl ZfsHealth {
    fn is_problematic(&self) -> bool {
        matches!(
            self,
            Self::Degraded | Self::Faulted | Self::Unavailable | Self::Suspended
        )
    }
}

string_enum!(ZfsVdevKind {
    Root => "root",
    Disk => "disk",
    File => "file",
    Mirror => "mirror",
    RaidZ => "raidz",
    DRaid => "draid",
    Replacing => "replacing",
    Spare => "spare",
});

string_enum!(ZfsVdevClass {
    Data => "data" | "normal",
    Special => "special",
    Dedup => "dedup",
    Log => "log",
    Cache => "cache",
    Spare => "spare",
});

string_enum!(ZfsScanKind {
    None => "NONE",
    Scrub => "SCRUB",
    Resilver => "RESILVER",
    ErrorScrub => "ERRORSCRUB",
});

string_enum!(ZfsScanState {
    None => "NONE",
    Scanning => "SCANNING",
    Finished => "FINISHED",
    Canceled => "CANCELED",
    ErrorScrubbing => "ERRORSCRUBBING",
});
