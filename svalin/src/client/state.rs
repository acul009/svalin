use std::collections::{BTreeMap, HashMap, HashSet};

use svalin_pki::{SpkiHash, mls::SvalinGroupId};

pub mod persistent;
pub mod tunneling;
pub mod warning;

pub use warning::Warning;

#[derive(Clone, Debug)]
pub struct ClientState {
    persistent: persistent::State,
    agents_online: HashSet<SpkiHash>,
    display_groups: BTreeMap<Option<String>, Vec<SpkiHash>>,
    device_group_broken: HashMap<SvalinGroupId, String>,
    warnings: warning::State,
    tunneling: tunneling::State,
}

#[derive(Clone, Debug)]
pub enum Update {
    Persistent(persistent::Update),
    AgentOnline(SpkiHash, bool),
    GroupBroken(SvalinGroupId, String),
    Tunnel(tunneling::Update),
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
            Self::Tunnel(_) => None,
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
        let mut display_groups = BTreeMap::new();
        for (spki_hash, device) in persistent.devices() {
            display_groups
                .entry(device.group_name().map(Into::into))
                .or_insert_with(Vec::new)
                .push(spki_hash.clone());
        }
        Self {
            persistent: persistent,
            agents_online: HashSet::new(),
            display_groups,
            device_group_broken: HashMap::new(),
            warnings: warning::State::new(),
            tunneling: tunneling::State::new(),
        }
    }

    pub fn update(&mut self, msg: Update) {
        let affected = msg.affected_device();
        match msg {
            Update::Persistent(msg) => {
                let affected = msg.affected_device();
                let previous_group = affected
                    .as_ref()
                    .map(|spki_hash| self.persistent.devices().get(&spki_hash))
                    .flatten()
                    .map(|device| device.group_name())
                    .flatten()
                    .cloned();

                self.persistent.update(msg);

                if let Some(affected) = affected {
                    let new_group = self
                        .persistent
                        .devices()
                        .get(&affected)
                        .map(|device| device.group_name())
                        .flatten()
                        .cloned();

                    if new_group != previous_group {
                        if let Some(old_group) = self.display_groups.get_mut(&previous_group) {
                            old_group.retain(|entry| entry != &affected);
                        }
                        self.display_groups
                            .entry(new_group)
                            .or_insert_with(Vec::new)
                            .push(affected);
                    }
                }
            }
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
            Update::Tunnel(update) => self.tunneling.update(update),
        }

        if let Some(device) = affected {
            let warnings = self.generate_device_warnings(&device);
            self.warnings.update_device(&device, warnings);
        }
    }

    pub fn agent_online(&self, spki_hash: &SpkiHash) -> bool {
        self.agents_online.contains(spki_hash)
    }

    pub fn groups(&self) -> impl Iterator<Item = &Option<String>> {
        self.display_groups.keys()
    }

    pub fn group(
        &self,
        group: &Option<String>,
    ) -> impl Iterator<Item = (&SpkiHash, &persistent::DeviceState)> {
        let Some(group) = self.display_groups.get(group) else {
            todo!("figure out nicer types here");
        };
        group.iter().map(|spki_hash| {
            (
                spki_hash,
                self.persistent
                    .devices
                    .get(spki_hash)
                    .expect("group referenced device should exist"),
            )
        })
    }

    pub fn persistent(&self) -> &persistent::State {
        &self.persistent
    }

    pub fn warnings(&self) -> &warning::State {
        &self.warnings
    }

    pub fn tunneling(&self) -> &tunneling::State {
        &self.tunneling
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
