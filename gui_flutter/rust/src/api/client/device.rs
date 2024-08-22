use std::ops::Deref;

use crate::frb_generated::StreamSink;
use flutter_rust_bridge::frb;
pub use svalin::client::device::Device;
pub use svalin::client::device::RealtimeStatusReceiver;
pub use svalin::client::device::RemoteLiveData;
pub use svalin::shared::commands::agent_list::AgentListItem;
pub use svalin::shared::join_agent::PublicAgentData;
pub use svalin_pki::Certificate;
pub use svalin_sysctl::realtime::CoreStatus;
pub use svalin_sysctl::realtime::CpuStatus;
pub use svalin_sysctl::realtime::MemoryStatus;
pub use svalin_sysctl::realtime::RealtimeStatus;
pub use svalin_sysctl::realtime::SwapStatus;

#[frb(external)]
impl Device {
    pub async fn item(&self) -> AgentListItem {}
    pub async fn subscribe_realtime(&self) -> RealtimeStatusReceiver {}
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
}

#[frb(external)]
impl RemoteLiveData<RealtimeStatus> {
    pub fn is_pending(&self) -> bool {}
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
