use std::collections::{HashMap, HashSet};

use svalin_client_store::persistent::{self};
use svalin_pki::SpkiHash;

#[derive(Clone, Debug)]
pub struct ClientState {
    persistent: persistent::State,
    agents_online: HashSet<SpkiHash>,
}

#[derive(Clone, Debug)]
pub enum ClientStateUpdate {
    Persistent(persistent::Message),
    AgentOnlineStatus(SpkiHash, bool),
}

impl ClientState {
    pub fn empty() -> Self {
        Self::new(persistent::State::empty())
    }

    pub fn new(persistent: persistent::State) -> Self {
        Self {
            persistent: persistent,
            agents_online: HashSet::new(),
        }
    }

    pub fn update(&mut self, msg: ClientStateUpdate) {
        match msg {
            ClientStateUpdate::Persistent(msg) => self.persistent.update(msg),
            ClientStateUpdate::AgentOnlineStatus(spki_hash, online) => {
                if online {
                    self.agents_online.insert(spki_hash);
                } else {
                    self.agents_online.remove(&spki_hash);
                }
            }
        }
    }

    pub fn agent_online(&self, spki_hash: &SpkiHash) -> bool {
        self.agents_online.contains(spki_hash)
    }

    pub fn persistent(&self) -> &HashMap<SpkiHash, persistent::DeviceState> {
        &self.persistent.devices()
    }
}
