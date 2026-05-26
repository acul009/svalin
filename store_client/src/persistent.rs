use std::{borrow::Cow, collections::HashMap, fmt::Debug};

use serde::{Deserialize, Serialize};
use svalin_pki::SpkiHash;
use svalin_sysctl::sytem_report::{OSFamily, SystemReport};

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
    UpdateSystemReport(SpkiHash, SvalinReport),
    UpdateMetaInfo(SpkiHash, SvalinMetaInfo),
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
                self.get_device_entry(spki_hash).report = Some(system_report)
            }
            Message::UpdateMetaInfo(spki_hash, meta_info) => {
                self.get_device_entry(spki_hash).meta_info = Some(meta_info)
            }
            Message::UpdateFromMainState(state) => {
                for (spki_hash, other_device) in state.devices {
                    let device = self.get_device_entry(spki_hash);
                    let current_report = device
                        .report()
                        .map(|r| r.system_report.generated_at)
                        .unwrap_or(0);
                    let other_report = other_device
                        .report()
                        .map(|r| r.system_report.generated_at)
                        .unwrap_or(0);
                    if current_report < other_report {
                        device.report = other_device.report;
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
                report: None,
                meta_info: None,
            })
    }

    pub fn devices(&self) -> &HashMap<SpkiHash, DeviceState> {
        &self.devices
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceState {
    spki_hash: SpkiHash,
    pub(crate) report: Option<SvalinReport>,
    pub(crate) meta_info: Option<SvalinMetaInfo>,
}

impl DeviceState {
    pub fn report(&self) -> Option<&SvalinReport> {
        self.report.as_ref()
    }

    pub fn meta_info(&self) -> Option<&SvalinMetaInfo> {
        self.meta_info.as_ref()
    }

    pub fn name(&self) -> Cow<'_, str> {
        if let Some(meta) = self.meta_info() {
            if !meta.name.is_empty() {
                return meta.name.as_str().into();
            }
        }
        if let Some(report) = self.report() {
            if let Some(hostname) = &report.system_report.hostname {
                return hostname.into();
            }
        }
        self.spki_hash.to_string().into()
    }

    pub fn os(&self) -> OSFamily {
        self.report()
            .map(|report| report.system_report.os_family)
            .unwrap_or(OSFamily::Unknown)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SvalinReport {
    pub current_version_identifiert: String,
    pub system_report: SystemReport,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SvalinMetaInfo {
    pub updated_at: u64,
    pub name: String,
    pub group: String,
    pub notes: String,
}
