use std::collections::HashSet;

use svalin_client_store::persistent;
use svalin_pki::SpkiHash;

#[derive(Clone)]
pub struct ClientState {
    persistent: persistent::State,
    agents_online: HashSet<SpkiHash>,
}

#[derive(Clone)]
pub enum ClientStateUpdate {
    Persistent(persistent::Message),
    AgentOnlineStatus(SpkiHash, bool),
}

impl ClientState {
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
}
