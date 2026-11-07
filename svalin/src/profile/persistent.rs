
use std::collections::{HashMap, HashSet};

use svalin_pki::SpkiHash;
use svalin_pki::mls::SvalinGroupId;
use svalin_pki::trust_store::TrustStore;

use crate::client::state::persistent::{MetaInfo, Report};

pub enum Update {
    GroupBroken(SvalinGroupId, String),
    GroupRestored(SvalinGroupId),
}

#[derive(Default)]
pub struct State {
    device_group_broken: HashMap<SvalinGroupId, String>,
    child_devices: HashSet<SpkiHash>,
    reports: HashMap<SpkiHash, Report>,
    meta_info: HashMap<SpkiHash, MetaInfo>,
}

impl State {
    pub fn group_error(&self, group: &SvalinGroupId) -> Option<&str> {
        self.device_group_broken.get(group).map(String::as_str)
    }

    pub fn is_child(&self, device: SpkiHash) -> bool {
        self.child_devices.contains(&device)
    }

    pub fn meta_info(&self, device: SpkiHash) -> Option<&MetaInfo> {
        self.meta_info.get(&device)
    }

    pub fn report(&self, device: SpkiHash) -> Option<&Report> {
        self.reports.get(&device)
    }

    pub fn update(&mut self, update: Update) -> SpkiHash {
        match update {
            Update::GroupBroken(group, error) => {
                let device = match &group {
                    SvalinGroupId::DeviceGroup(device)
                    | SvalinGroupId::DeviceMetaGroup(device) => device.clone(),
                };
                self.device_group_broken.insert(group, error);
                device
            }
            Update::GroupRestored(group) => {
                self.device_group_broken.remove(&group);
                match group {
                    SvalinGroupId::DeviceGroup(device)
                    | SvalinGroupId::DeviceMetaGroup(device) => device,
                }
            }
        }
    }

    pub fn update_from_trust_store(
        &mut self,
        trust_store: &TrustStore,
        user: &SpkiHash,
    ) -> Vec<SpkiHash> {
        let child_devices: HashSet<SpkiHash> = trust_store
            .child_devices(user)
            .map(|certificate| certificate.spki_hash().clone())
            .collect();

        let changed = self
            .child_devices
            .symmetric_difference(&child_devices)
            .cloned()
            .collect();

        // TODO: Decide when removed devices should also lose their metadata and reports.
        self.child_devices = child_devices;
        changed

    }
}
