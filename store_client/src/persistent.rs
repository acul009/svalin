use std::{borrow::Cow, collections::HashMap, fmt::Debug};

use serde::{Deserialize, Serialize};
use svalin_pki::SpkiHash;
use svalin_sysctl::sytem_report::{OS, SystemReport};

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
                self.get_device_entry(spki_hash).system_report = Some(system_report)
            }
            Message::UpdateFromMainState(state) => {
                for (spki_hash, other_device) in state.devices {
                    let device = self.get_device_entry(spki_hash);
                    let current_report =
                        device.system_report().map(|r| r.generated_at).unwrap_or(0);
                    let other_report = other_device
                        .system_report()
                        .map(|r| r.generated_at)
                        .unwrap_or(0);
                    if current_report < other_report {
                        device.system_report = other_device.system_report;
                    }
                }
            }
        }
    }

    fn get_device_entry(&mut self, spki_hash: SpkiHash) -> &mut DeviceState {
        self.devices
            .entry(spki_hash.clone())
            .or_insert_with(|| DeviceState {
                spki_hash,
                system_report: None,
            })
    }

    pub fn devices(&self) -> &HashMap<SpkiHash, DeviceState> {
        &self.devices
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceState {
    spki_hash: SpkiHash,
    pub(crate) system_report: Option<SystemReport>,
}

impl DeviceState {
    pub fn system_report(&self) -> Option<&SystemReport> {
        self.system_report.as_ref()
    }

    pub fn name(&self) -> Cow<'_, str> {
        self.spki_hash.to_string().into()
    }

    pub fn os(&self) -> OS {
        self.system_report()
            .map(|report| report.os)
            .unwrap_or(OS::Unknown)
    }
}
