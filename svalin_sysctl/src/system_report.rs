use std::{
    fmt::Display,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

pub mod proxmox_bs;
pub mod proxmox_mg;
pub mod proxmox_ve;
pub mod windows;
pub mod zfs;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemReport {
    pub generated_at: u64,
    pub os_family: OSFamily,
    pub os: Option<String>,
    pub kernel_version: String,
    pub hostname: Option<String>,
    pub cpu: Cpu,
    pub total_memory: u64,
    pub total_swap: u64,
    pub disks: Vec<Disk>,
    pub extensions: Extensions,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Extensions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    proxmox_ve: Option<proxmox_ve::ProxmoxVE>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    proxmox_mg: Option<proxmox_mg::ProxmoxMG>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    proxmox_bs: Option<proxmox_bs::ProxmoxBS>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    windows: Option<windows::Windows>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zfs: Option<zfs::Zfs>,
}

impl Extensions {
    pub fn zfs(&self) -> Option<&zfs::Zfs> {
        self.zfs.as_ref()
    }

    pub fn proxmox_ve(&self) -> Option<&proxmox_ve::ProxmoxVE> {
        self.proxmox_ve.as_ref()
    }

    pub fn proxmox_mg(&self) -> Option<&proxmox_mg::ProxmoxMG> {
        self.proxmox_mg.as_ref()
    }

    pub fn proxmox_bs(&self) -> Option<&proxmox_bs::ProxmoxBS> {
        self.proxmox_bs.as_ref()
    }

    pub fn windows(&self) -> Option<&windows::Windows> {
        self.windows.as_ref()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Cpu {
    pub brand: String,
    pub model: String,
    pub cores: Option<usize>,
    pub arch: String,
    pub threads: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Disk {
    pub name: String,
    pub file_system: String,
    pub mount_point: String,
    pub total_space: u64,
    pub available_space: u64,
    pub kind: sysinfo::DiskKind,
}

/// Owns the system state used for periodic reports, independently of live data.
#[derive(Default)]
pub struct SystemReporter {
    system: Option<sysinfo::System>,
}

impl SystemReporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lazily initializes the system. Cancelling its blocking refresh resets sampling history.
    pub async fn create(&mut self) -> anyhow::Result<SystemReport> {
        let system = self.system.take();
        let (system, report) = tokio::task::spawn_blocking(move || {
            let mut system = system.unwrap_or_default();
            let report = Self::collect(&mut system);
            (system, report)
        })
        .await?;
        self.system = Some(system);
        let mut report = report?;
        report.extensions.proxmox_ve = proxmox_ve::ProxmoxVE::create().await?;
        report.extensions.proxmox_bs = proxmox_bs::ProxmoxBS::create().await?;
        report.extensions.proxmox_mg = proxmox_mg::ProxmoxMG::create().await?;
        report.extensions.windows = windows::Windows::create().await;
        report.extensions.zfs = zfs::Zfs::create().await;
        Ok(report)
    }
    fn collect(sys: &mut sysinfo::System) -> anyhow::Result<SystemReport> {
        sys.refresh_specifics(
            sysinfo::RefreshKind::nothing()
                .with_cpu(sysinfo::CpuRefreshKind::everything())
                .with_memory(sysinfo::MemoryRefreshKind::everything()),
        );

        #[cfg(windows)]
        let os = OSFamily::Windows;

        #[cfg(unix)]
        let os = OSFamily::Linux;

        #[cfg(all(not(unix), not(windows)))]
        let os = OSFamily::Unknown;

        let generated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs();

        let cpu = sys.cpus().first().ok_or(anyhow::anyhow!("no cpu found"))?;
        let cpu = Cpu {
            brand: cpu.vendor_id().trim().to_string(),
            model: cpu.brand().trim().to_string(),
            arch: sysinfo::System::cpu_arch(),
            cores: sysinfo::System::physical_core_count(),
            threads: sys.cpus().len(),
        };

        let mut disks = sysinfo::Disks::new_with_refreshed_list()
            .iter()
            .map(|disk| Disk {
                name: disk.name().to_string_lossy().to_string(),
                kind: disk.kind(),
                file_system: disk.file_system().to_string_lossy().to_string(),
                mount_point: disk.mount_point().to_string_lossy().to_string(),
                total_space: disk.total_space(),
                available_space: disk.available_space(),
            })
            // ignore docker overlays
            .filter(|disk| {
                !disk.mount_point.starts_with("/var/lib/docker") && disk.file_system != "overlay"
            })
            .collect::<Vec<_>>();
        disks.sort_by_cached_key(|disk| disk.mount_point.clone());

        Ok(SystemReport {
            os_family: os,
            os: sysinfo::System::long_os_version(),
            kernel_version: sysinfo::System::kernel_long_version(),
            hostname: sysinfo::System::host_name(),
            generated_at,
            cpu,
            total_memory: sys.total_memory(),
            total_swap: sys.total_swap(),
            disks,
            extensions: Extensions::default(),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum OSFamily {
    Windows,
    Linux,
    Unknown,
}

impl Display for OSFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OSFamily::Windows => write!(f, "Windows"),
            OSFamily::Linux => write!(f, "Linux"),
            OSFamily::Unknown => write!(f, "Unknown"),
        }
    }
}
