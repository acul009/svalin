use std::{collections::HashMap, fmt::Debug};

use serde::{Deserialize, Serialize};
use svalin_pki::SpkiHash;
use svalin_sysctl::sytem_report::SystemReport;

/// This contains the persistent state of the clients available information.
/// It is not meant to contain live information like current cpu usage or online status.
/// This should only contain data which is still relevant after a device has been shut down.
///
/// That also entails that this state should be updated whether the user looks at it or not.
/// Live data, in contrast, should only be updated when the user actively interacts with it - it's ephemeral.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct State {
    pub(crate) devices: HashMap<SpkiHash, DeviceState>,
}

#[derive(Clone, Debug)]
pub enum Message {
    UpdateSystemReport(SpkiHash, SystemReport),
    UpdateFromMainState(State),
}

impl State {
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
                        system_report: system_report,
                    });
            }
            Message::UpdateFromMainState(state) => {
                for (spki_hash, other_device) in state.devices {
                    if let Some(my_device) = self.devices.get_mut(&spki_hash) {
                        if other_device.system_report.generated_at
                            > my_device.system_report.generated_at
                        {
                            my_device.system_report = other_device.system_report;
                        }
                    } else {
                        self.devices.insert(spki_hash, other_device);
                    }
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceState {
    pub(crate) system_report: SystemReport,
}
