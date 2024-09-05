use flutter_rust_bridge::frb;
pub use svalin::client::device::Device;
pub use svalin::client::device::RealtimeStatusReceiver;
pub use svalin::client::device::RemoteLiveData;
pub use svalin::shared::commands::agent_list::AgentListItem;
pub use svalin::shared::commands::terminal::RemoteTerminal;
pub use svalin::shared::join_agent::PublicAgentData;
pub use svalin_pki::Certificate;
pub use svalin_sysctl::pty::TerminalSize;
pub use svalin_sysctl::realtime::CoreStatus;
pub use svalin_sysctl::realtime::CpuStatus;
pub use svalin_sysctl::realtime::MemoryStatus;
pub use svalin_sysctl::realtime::RealtimeStatus;
pub use svalin_sysctl::realtime::SwapStatus;
pub use tokio::sync::watch::error::RecvError;

#[frb(external)]
impl Device {
    pub async fn item(&self) -> AgentListItem {}
    pub async fn subscribe_realtime(&self) -> RealtimeStatusReceiver {}
    pub async fn open_terminal(&self) -> anyhow::Result<RemoteTerminal> {}
}

#[frb(non_opaque, mirror(AgentListItem))]
pub struct _AgentListItem {
    pub public_data: PublicAgentData,
    pub online_status: bool,
}

#[frb(non_opaque, mirror(PublicAgentData))]
pub struct _PublicAgentData {
    pub name: String,
    pub cert: Certificate,
}

#[frb(external)]
impl RealtimeStatusReceiver {
    pub fn current_owned(&self) -> RemoteLiveData<RealtimeStatus> {}
    pub async fn changed(&mut self) -> Result<(), RecvError> {}
}

#[frb(external)]
impl RemoteLiveData<RealtimeStatus> {
    #[frb(sync)]
    pub fn is_pending(&self) -> bool {}
    #[frb(sync)]
    pub fn is_unavailable(&self) -> bool {}
    #[frb(sync)]
    pub fn get_ready(self) -> Option<RealtimeStatus> {}
}

// impl From<&RemoteLiveData<RealtimeStatus>> for RemoteLiveDataRealtimeStatus {
//     fn from(value: &RemoteLiveData<RealtimeStatus>) -> Self {
//         match value {
//             RemoteLiveData::Unavailable => Self::Unavailable,
//             RemoteLiveData::Pending => Self::Pending,
//             RemoteLiveData::Ready(value) => Self::Ready(value.clone()),
//         }
//     }
// }

// pub async fn device_subscribe_realtime_status(
//     sink: StreamSink<RemoteLiveDataRealtimeStatus>,
//     device: Device,
// ) {
//     let mut watch = device.subscribe_realtime().await;
//     sink.add(watch.borrow().deref().into()).unwrap();

//     while let Ok(_) = watch.changed().await {
//         if let Err(_) = sink.add(watch.borrow().deref().into()) {
//             return;
//         };
//     }
// }

#[frb(non_opaque, mirror(RealtimeStatus))]
pub struct _RealtimeStatus {
    pub cpu: CpuStatus,
    pub memory: MemoryStatus,
    pub swap: SwapStatus,
}

#[frb(non_opaque, mirror(CpuStatus))]
pub struct _CpuStatus {
    pub cores: Vec<CoreStatus>,
}

#[frb(non_opaque, mirror(CoreStatus))]
pub struct _CoreStatus {
    pub load: f32,
    pub frequency: u64,
}

#[frb(non_opaque, mirror(MemoryStatus))]
pub struct _MemoryStatus {
    pub total: u64,
    pub available: u64,
    pub free: u64,
    pub used: u64,
}

#[frb(non_opaque, mirror(SwapStatus))]
pub struct _SwapStatus {
    pub total: u64,
    pub free: u64,
    pub used: u64,
}

#[frb(external)]
impl RemoteTerminal {
    pub async fn write(&self, content: String) {}
    pub async fn resize(&self, size: TerminalSize) {}
    pub async fn read(&self) -> anyhow::Result<Option<String>> {}
}

#[frb(non_opaque, mirror(TerminalSize))]
pub struct _TerminalSize {
    pub cols: u16,
    pub rows: u16,
}
