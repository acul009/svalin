/// Module to collect real-time system status metrics
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use tokio::sync::Mutex;

/// Struct representing the overall real-time system status
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RealtimeStatus {
    pub cpu: CpuStatus,
    pub memory: MemoryStatus,
    pub swap: SwapStatus,
}

/// CPU status containing per-core information
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CpuStatus {
    pub cores: Vec<CoreStatus>,
}

/// Status for each CPU core
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CoreStatus {
    pub load: f32,       // current CPU usage in percentage
    pub frequency: u64,  // current frequency in MHz
}

/// Memory usage status
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemoryStatus {
    pub total: u64,
    pub available: u64,
    pub free: u64,
    pub used: u64,
}

/// Swap usage status
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SwapStatus {
    pub total: u64,
    pub free: u64,
    pub used: u64,
}

/// Global static system object protected by mutex
static SYS: LazyLock<Mutex<System>> = LazyLock::new(|| Mutex::new(System::new()));

impl RealtimeStatus {
    /// Retrieves the current system status snapshot asynchronously
    pub async fn get() -> RealtimeStatus {
        let mut sys_lock = SYS.lock().await;

        // Refresh CPU and memory information
        sys_lock.refresh_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );

        // Collect per-core CPU status
        let cores: Vec<CoreStatus> = sys_lock
            .cpus()
            .iter()
            .map(|cpu| CoreStatus {
                load: cpu.cpu_usage(),
                frequency: cpu.frequency(),
            })
            .collect();

        // Return collected snapshot
        RealtimeStatus {
            cpu: CpuStatus { cores },
            memory: MemoryStatus {
                total: sys_lock.total_memory(),
                available: sys_lock.available_memory(),
                free: sys_lock.free_memory(),
                used: sys_lock.used_memory(),
            },
            swap: SwapStatus {
                total: sys_lock.total_swap(),
                free: sys_lock.free_swap(),
                used: sys_lock.used_swap(),
            },
        }
    }
}
