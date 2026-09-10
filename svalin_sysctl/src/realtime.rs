use serde::{Deserialize, Serialize};
use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System,
    UpdateKind,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RealtimeStatus {
    pub cpu: CpuStatus,
    pub memory: MemoryStatus,
    pub swap: SwapStatus,
    #[serde(default)]
    pub processes: Vec<ProcessInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub executable: Option<String>,
    pub status: sysinfo::ProcessStatus,
    /// Seconds since the Unix epoch; distinguishes reused PIDs.
    pub start_time: u64,
    /// Elapsed lifetime in seconds.
    pub run_time: u64,
    /// CPU percentage, where 100% is one fully used logical core.
    /// Needs two refreshes separated by sysinfo::MINIMUM_CPU_UPDATE_INTERVAL;
    /// the first sample of a newly observed process is not yet meaningful.
    pub cpu_usage: f32,
    /// Resident memory in bytes.
    pub memory: u64,
    /// Virtual memory in bytes.
    pub virtual_memory: u64,
    /// Total and since-last-refresh byte counters, not bytes per second.
    /// On Windows these include all I/O, not only disk I/O.
    pub io: ProcessIoUsage,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessIoUsage {
    pub total_read_bytes: u64,
    pub total_written_bytes: u64,
    /// Bytes read since the previous refresh.
    pub read_bytes: u64,
    /// Bytes written since the previous refresh.
    pub written_bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CpuStatus {
    pub cores: Vec<CoreStatus>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CoreStatus {
    pub load: f32,
    pub frequency: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemoryStatus {
    pub total: u64,
    pub available: u64,
    pub free: u64,
    pub used: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SwapStatus {
    pub total: u64,
    pub free: u64,
    pub used: u64,
}

/// Owns the sampling history for one live data stream.
/// Keep this reporter between polls; separate reporters have independent samples.
#[derive(Default)]
pub struct RealtimeReporter {
    system: System,
}

impl RealtimeReporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves collection onto a blocking worker and returns the reporter for reuse.
    pub async fn get(mut self) -> (Self, RealtimeStatus) {
        tokio::task::spawn_blocking(move || {
            let status = Self::collect(&mut self.system);
            (self, status)
        })
        .await
        .expect("realtime status collection task panicked")
    }

    fn collect(sys: &mut System) -> RealtimeStatus {
        sys.refresh_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_cpu()
                .with_memory()
                .with_disk_usage()
                .with_exe(UpdateKind::OnlyIfNotSet),
        );

        let cores: Vec<CoreStatus> = sys
            .cpus()
            .iter()
            .map(|cpu| CoreStatus {
                load: cpu.cpu_usage(),
                frequency: cpu.frequency(),
            })
            .collect();

        let mut processes: Vec<_> = sys
            .processes()
            .values()
            .map(|process| ProcessInfo {
                pid: process.pid().as_u32(),
                parent_pid: process.parent().map(|pid| pid.as_u32()),
                name: process.name().to_string_lossy().into_owned(),
                executable: process
                    .exe()
                    .map(|path| path.to_string_lossy().into_owned()),
                status: process.status(),
                start_time: process.start_time(),
                run_time: process.run_time(),
                cpu_usage: process.cpu_usage(),
                memory: process.memory(),
                virtual_memory: process.virtual_memory(),
                io: {
                    let usage = process.disk_usage();
                    ProcessIoUsage {
                        total_read_bytes: usage.total_read_bytes,
                        total_written_bytes: usage.total_written_bytes,
                        read_bytes: usage.read_bytes,
                        written_bytes: usage.written_bytes,
                    }
                },
            })
            .collect();
        processes.sort_unstable_by_key(|process| process.pid);

        RealtimeStatus {
            cpu: CpuStatus { cores },
            memory: MemoryStatus {
                total: sys.total_memory(),
                available: sys.available_memory(),
                free: sys.free_memory(),
                used: sys.used_memory(),
            },
            swap: SwapStatus {
                total: sys.total_swap(),
                free: sys.free_swap(),
                used: sys.used_swap(),
            },
            processes,
        }
    }
}
