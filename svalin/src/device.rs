use serde::{Deserialize, Serialize};

use crate::client::tunnel_manager::TunnelConfig;

#[derive(Serialize, Deserialize)]
pub struct AssociatedDeviceData {
    device_name: String,
    device_group: String,
    saved_tunnels: Vec<TunnelConfig>,
}
