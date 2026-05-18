use std::{
    fmt::Display,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

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

impl SystemReport {
    pub async fn create() -> anyhow::Result<Self> {
        Ok(tokio::task::spawn_blocking(|| Self::create_inner()).await??)
    }
    pub fn create_inner() -> anyhow::Result<Self> {
        let sys = sysinfo::System::new_all();

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
            .collect::<Vec<_>>();
        disks.sort_by_cached_key(|disk| disk.mount_point.clone());

        Ok(Self {
            os_family: os,
            os: sysinfo::System::long_os_version(),
            kernel_version: sysinfo::System::kernel_long_version(),
            hostname: sysinfo::System::host_name(),
            generated_at,
            cpu,
            total_memory: sys.total_memory(),
            total_swap: sys.total_swap(),
            disks,
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
