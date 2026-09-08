use std::collections::{HashMap, HashSet};

use svalin_pki::{SpkiHash, mls::SvalinGroupId};

pub mod persistent;
// pub mod tunneling;
pub mod warning;

pub use warning::Warning;

#[derive(Clone, Debug)]
pub struct ClientState {
    persistent: persistent::State,
    agents_online: HashSet<SpkiHash>,
    device_group_broken: HashMap<SvalinGroupId, String>,
    warnings: warning::State,
}

#[derive(Clone, Debug)]
pub enum Update {
    Persistent(persistent::Update),
    AgentOnline(SpkiHash, bool),
    GroupBroken(SvalinGroupId, String),
}

impl Update {
    pub fn affected_device(&self) -> Option<SpkiHash> {
        match self {
            Self::Persistent(update) => update.affected_device(),
            Self::AgentOnline(spki_hash, _) => Some(spki_hash.clone()),
            Self::GroupBroken(group_id, _) => match group_id {
                SvalinGroupId::DeviceGroup(spki_hash) => Some(spki_hash.clone()),
                SvalinGroupId::DeviceMetaGroup(spki_hash) => Some(spki_hash.clone()),
            },
        }
    }
}

impl From<persistent::Update> for Update {
    fn from(msg: persistent::Update) -> Self {
        Self::Persistent(msg)
    }
}

impl ClientState {
    pub fn empty() -> Self {
        Self::new(persistent::State::empty())
    }

    pub fn new(persistent: persistent::State) -> Self {
        Self {
            persistent: persistent,
            agents_online: HashSet::new(),
            device_group_broken: HashMap::new(),
            warnings: warning::State::new(),
        }
    }

    pub fn update(&mut self, msg: Update) {
        let affected = msg.affected_device();
        match msg {
            Update::Persistent(msg) => self.persistent.update(msg),
            Update::AgentOnline(spki_hash, online) => {
                if online {
                    self.agents_online.insert(spki_hash);
                } else {
                    self.agents_online.remove(&spki_hash);
                }
            }
            Update::GroupBroken(group, reason) => {
                self.device_group_broken.insert(group, reason);
            }
        }

        if let Some(device) = affected {
            let warnings = self.generate_device_warnings(&device);
            self.warnings.update_device(&device, warnings);
        }
    }

    pub fn agent_online(&self, spki_hash: &SpkiHash) -> bool {
        self.agents_online.contains(spki_hash)
    }

    pub fn persistent(&self) -> &persistent::State {
        &self.persistent
    }

    pub fn warnings(&self) -> &warning::State {
        &self.warnings
    }

    fn generate_device_warnings(&self, spki_hash: &SpkiHash) -> Vec<warning::Device> {
        let mut warnings: Vec<warning::Device> = self
            .persistent()
            .generate_device_warnings(spki_hash)
            .into_iter()
            .collect();

        if let Some(error_message) = self
            .device_group_broken
            .get(&SvalinGroupId::DeviceGroup(spki_hash.clone()))
        {
            warnings.push(warning::Device::DeviceGroupBroken(error_message.clone()));
        }

        warnings
    }
}
