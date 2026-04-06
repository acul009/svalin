use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use svalin_pki::SpkiHash;
use svalin_sysctl::sytem_report::SystemReport;

/// This contains the persistent state of the clients available information.
/// It is not meant to contain live information like current cpu usage or online status.
/// This should only contain data which is still relevant after a device has been shut down.
///
/// That also entails that this state should be updated whether the user looks at it or not.
/// Live data, in contrast, should only be updated when the user actively interacts with it - it's ephemeral.
#[derive(Serialize, Deserialize)]
pub struct ClientState {
    pub(crate) devices: HashMap<SpkiHash, DeviceState>,
}

pub enum Message {
    UpdateSystemReport(SpkiHash, SystemReport),
}

impl ClientState {
    pub fn empty() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::UpdateSystemReport(spki_hash, system_report) => {
                self.devices
                    .entry(spki_hash)
                    .or_insert_with(|| DeviceState {
                        system_report: Some(system_report),
                    });
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct DeviceState {
    system_report: Option<SystemReport>,
}
