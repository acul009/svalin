use std::{
    sync::{atomic::AtomicU64, LazyLock},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize, Clone)]
pub struct RealtimeStatus {
    pub cpu: CpuStatus,
    pub memory: MemoryStatus,
    pub swap: SwapStatus,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CpuStatus {
    pub cores: Vec<CoreStatus>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CoreStatus {
    pub load: f32,
    pub frequency: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MemoryStatus {
    pub total: u64,
    pub available: u64,
    pub free: u64,
    pub used: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SwapStatus {
    pub total: u64,
    pub free: u64,
    pub used: u64,
}

static SYS: LazyLock<Mutex<System>> = LazyLock::new(|| Mutex::new(System::new()));

impl RealtimeStatus {
    pub async fn get() -> RealtimeStatus {
        let mut sys_lock = SYS.lock().await;

        sys_lock.refresh_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                .with_memory(MemoryRefreshKind::everything()),
        );

        let cores: Vec<CoreStatus> = sys_lock
            .cpus()
            .iter()
            .map(|cpu| CoreStatus {
                load: cpu.cpu_usage(),
                frequency: cpu.frequency(),
            })
            .collect();

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
