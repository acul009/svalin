use crate::frb_generated::StreamSink;
use flutter_rust_bridge::frb;
pub use svalin::client::device::Device;
pub use svalin::client::device::RemoteLiveData;
pub use svalin::shared::commands::agent_list::AgentListItem;
pub use svalin::shared::join_agent::PublicAgentData;
pub use svalin_pki::Certificate;
pub use svalin_sysctl::realtime::CpuStatus;
pub use svalin_sysctl::realtime::MemoryStatus;
pub use svalin_sysctl::realtime::RealtimeStatus;
pub use svalin_sysctl::realtime::SwapStatus;

#[frb(external)]
impl Device {
    pub async fn item(&self) -> AgentListItem {}
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

pub type RemoteLiveDataRealtimeStatus = RemoteLiveData<RealtimeStatus>;

#[frb(non_opaque, mirror(RemoteLiveDataRealtimeStatus))]
pub enum _RemoteLiveDataRealtimeStatus {
    Unavailable,
    Pending,
    Ready(RealtimeStatus),
}

#[frb(non_opaque, mirror(RealtimeStatus))]
pub struct _RealtimeStatus {
    pub cpu: CpuStatus,
    pub memory: MemoryStatus,
    pub swap: SwapStatus,
}

pub async fn device_subscribe_realtime_status(
    sink: StreamSink<RemoteLiveDataRealtimeStatus>,
    device: Device,
) {
    let mut watch = device.subscribe_realtime().await;
    sink.add(watch.borrow().clone()).unwrap();

    while let Ok(_) = watch.changed().await {
        if let Err(_) = sink.add(watch.borrow().clone()) {
            return;
        };
    }
}
