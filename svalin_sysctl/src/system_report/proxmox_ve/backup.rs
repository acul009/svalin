//! Proxmox VE backup reporting.
//!
//! Backup job definitions come from `/cluster/backup`. Node-local scheduler
//! state in `/var/lib/pve-manager/jobs/vzdump-{job_id}.json` links each job
//! to its latest task UPID and result.
//!
//! Tracks scheduled executions, not the continued existence of backup archives.

use std::{collections::HashMap, time::Duration};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use super::query;

pub mod coverage;

pub const BACKUP_OVERDUE_GRACE_SECONDS: u64 = 2 * 60 * 60;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupJob {
    pub id: String,
    /// None when the job inherits its destination from vzdump defaults.
    pub storage: Option<String>,
    pub schedule: String,
    /// Unix seconds; unavailable when no reliable scheduling baseline exists.
    pub due_at: Option<u64>,
    pub status: BackupStatus,
    /// Current configured guests and volumes, not proof of a completed backup.
    #[serde(default)]
    pub coverage: coverage::Coverage,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BackupStatus {
    Running {
        started_at: u64,
    },
    Failed {
        /// Unix seconds, if the completion time is still available.
        finished_at: Option<u64>,
        message: String,
    },
    Succeeded {
        finished_at: Option<u64>,
    },
    Unknown,
}

impl BackupJob {
    /// Running jobs are not considered overdue by this scheduling check.
    /// Unknown due times cannot establish that a job is overdue.
    pub fn is_overdue(&self, now: u64) -> bool {
        !matches!(self.status, BackupStatus::Running { .. })
            && self
                .due_at
                .is_some_and(|due| now > due.saturating_add(BACKUP_OVERDUE_GRACE_SECONDS))
    }
}

pub(super) async fn collect() -> Option<Vec<BackupJob>> {
    match collect_jobs().await {
        Ok(jobs) => Some(jobs),
        Err(error) => {
            tracing::warn!(%error, "failed to collect PVE backup jobs");
            None
        }
    }
}

#[derive(Deserialize)]
struct JobConfig {
    id: String,
    enabled: Option<u8>,
    node: Option<String>,
    storage: Option<String>,
    schedule: String,
}

#[derive(Deserialize)]
struct JobState {
    state: String,
    upid: Option<String>,
    msg: Option<String>,
    time: Option<u64>,
}

#[derive(Deserialize)]
struct Task {
    upid: String,
    starttime: u64,
    endtime: Option<u64>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct TaskStatus {
    status: String,
    starttime: u64,
    exitstatus: Option<String>,
}

#[derive(Deserialize)]
struct ScheduleEvent {
    timestamp: u64,
}

async fn collect_jobs() -> anyhow::Result<Vec<BackupJob>> {
    let configs: Vec<JobConfig> = query("/cluster/backup", &[]).await?;
    if configs.is_empty() {
        return Ok(Vec::new());
    }
    // pmxcfs points this symlink at nodes/<local-node>, avoiding hostname aliases.
    let local = tokio::fs::read_link("/etc/pve/local").await?;
    let node = local
        .file_name()
        .and_then(|name| name.to_str())
        .context("invalid local PVE node name")?;
    let configs: Vec<_> = configs
        .into_iter()
        .filter(|job| job.enabled != Some(0) && job.node.as_deref().is_none_or(|name| name == node))
        .collect();
    if configs.is_empty() {
        return Ok(Vec::new());
    }

    let mut states = Vec::new();
    for config in configs {
        let state = match read_state(&config.id).await {
            Ok(state) => Some(state),
            Err(error) => {
                tracing::warn!(job = %config.id, %error, "PVE backup scheduler state unavailable");
                None
            }
        };
        states.push((config, state));
    }
    let wanted: Vec<_> = states
        .iter()
        .filter_map(|(_, state)| state.as_ref().and_then(|state| state.upid.as_deref()))
        .collect();
    let history = task_history(node, &wanted).await;
    let mut jobs = Vec::new();
    for (config, state) in states {
        let (status, baseline) = last_run(node, &config.id, state.as_ref(), &history).await;
        let due_at = next_due(&config, baseline).await;
        let coverage = query(
            &format!("/cluster/backup/{}/included_volumes", config.id),
            &[],
        )
        .await
        .with_context(|| format!("failed to collect PVE backup coverage for {}", config.id))?;
        jobs.push(BackupJob {
            id: config.id,
            storage: config.storage,
            schedule: config.schedule,
            due_at,
            status,
            coverage,
        });
    }
    Ok(jobs)
}

async fn last_run(
    node: &str,
    job: &str,
    state: Option<&JobState>,
    history: &HashMap<String, Task>,
) -> (BackupStatus, Option<u64>) {
    let Some(state) = state else {
        return (BackupStatus::Unknown, None);
    };
    let Some(upid) = &state.upid else {
        let status = if state.state == "stopped" {
            result_status(state.msg.as_deref(), state.time)
        } else {
            BackupStatus::Unknown
        };
        return (status, state.time.filter(|time| *time > 0));
    };
    let archived = history.get(upid);
    if let Some(task) = archived {
        if task.endtime.is_some()
            && task
                .status
                .as_deref()
                .is_some_and(|s| !s.is_empty() && s != "RUNNING")
        {
            return (
                result_status(task.status.as_deref(), task.endtime),
                Some(task.starttime),
            );
        }
    }
    match query::<TaskStatus>(&format!("/nodes/{node}/tasks/{upid}/status"), &[]).await {
        Ok(task) => {
            let status = match task.status.as_str() {
                "running" => BackupStatus::Running {
                    started_at: task.starttime,
                },
                "stopped" => result_status(
                    task.exitstatus.as_deref(),
                    archived.and_then(|task| task.endtime),
                ),
                _ => BackupStatus::Unknown,
            };
            (status, Some(task.starttime))
        }
        Err(error) => {
            tracing::warn!(job, %error, "PVE backup task status unavailable");
            let status = if state.state == "stopped" {
                result_status(state.msg.as_deref(), archived.and_then(|task| task.endtime))
            } else {
                BackupStatus::Unknown
            };
            (
                status,
                archived
                    .map(|task| task.starttime)
                    .or_else(|| upid_starttime(upid)),
            )
        }
    }
}

async fn next_due(config: &JobConfig, baseline: Option<u64>) -> Option<u64> {
    let start = baseline?.to_string();
    // pvesh can pass starttime to the Rust calendar binding as a string.
    // Use the same calendar library with an explicit integer conversion.
    // TODO: Make report to proxmox to fix their conversion of pvesh
    let result: anyhow::Result<Vec<ScheduleEvent>> = async {
        let output = tokio::time::timeout(
            Duration::from_secs(30),
            Command::new("/usr/bin/perl")
                .args([
                    "-MPVE::CalendarEvent",
                    "-MJSON",
                    "-e",
                    r#"
my ($schedule, $start) = @ARGV;
die "Expected a UNIX timestamp\n"
    unless defined($start) && $start =~ /^\d+$/;
my $event = PVE::CalendarEvent::parse_calendar_event($schedule);
my $next = PVE::CalendarEvent::compute_next_event($event, int($start));
print encode_json(defined($next) ? [{ timestamp => $next }] : []), "\n";
"#,
                    "--",
                    &config.schedule,
                    &start,
                ])
                .kill_on_drop(true)
                .output(),
        )
        .await
        .context("PVE calendar calculation timed out")?
        .context("failed to run PVE calendar calculation")?;
        anyhow::ensure!(
            output.status.success(),
            "PVE calendar calculation failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        serde_json::from_slice(&output.stdout).context("invalid PVE calendar response")
    }
    .await;
    match result {
        Ok(events) => events.first().map(|event| event.timestamp),
        Err(error) => {
            tracing::warn!(job = %config.id, %error, "PVE backup due time unavailable");
            None
        }
    }
}

fn result_status(result: Option<&str>, finished_at: Option<u64>) -> BackupStatus {
    match result {
        Some("OK") => BackupStatus::Succeeded { finished_at },
        Some(message) if !message.trim().is_empty() => BackupStatus::Failed {
            finished_at,
            message: message.to_owned(),
        },
        _ => BackupStatus::Unknown,
    }
}

async fn read_state(id: &str) -> anyhow::Result<JobState> {
    anyhow::ensure!(
        !id.is_empty()
            && id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c)),
        "invalid backup job ID"
    );
    let data = tokio::fs::read(format!("/var/lib/pve-manager/jobs/vzdump-{id}.json")).await?;
    Ok(serde_json::from_slice(&data)?)
}

fn upid_starttime(upid: &str) -> Option<u64> {
    if !upid.starts_with("UPID:") {
        return None;
    }
    u64::from_str_radix(upid.split(':').nth(4)?, 16).ok()
}

async fn task_history(node: &str, wanted: &[&str]) -> HashMap<String, Task> {
    let mut history = HashMap::new();
    if wanted.is_empty() {
        return history;
    }
    // Bound collection even on nodes with extensive history. Missing entries
    // leave completion timestamps unknown; they do not imply failed backups.
    for page in 0..20 {
        let start = (page * 100).to_string();
        let tasks = query::<Vec<Task>>(
            &format!("/nodes/{node}/tasks"),
            &[
                "--typefilter",
                "vzdump",
                "--source",
                "all",
                "--limit",
                "100",
                "--start",
                &start,
            ],
        )
        .await;
        let tasks = match tasks {
            Ok(tasks) => tasks,
            Err(error) => {
                tracing::warn!(%error, "PVE backup task history unavailable");
                break;
            }
        };
        let count = tasks.len();
        for task in tasks {
            if wanted.contains(&task.upid.as_str()) {
                history.insert(task.upid.clone(), task);
            }
        }
        if count < 100 || wanted.iter().all(|upid| history.contains_key(*upid)) {
            break;
        }
    }
    history
}
