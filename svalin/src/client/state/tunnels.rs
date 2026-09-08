use std::collections::HashMap;

use svalin_pki::SpkiHash;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct State {
    active: HashMap<SpkiHash, HashMap<Uuid, TunnelDefinition>>,
}

pub enum Update {
    Opened(SpkiHash, Uuid, TunnelDefinition),
    Closed(SpkiHash, Uuid),
}

impl State {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
        }
    }

    pub fn update(&mut self, update: Update) {
        match update {
            Update::Opened(peer, id, tunnel) => {
                self.active.entry(peer).or_default().insert(id, tunnel);
            }
            Update::Closed(peer, id) => {
                if let Some(tunnels) = self.active.get_mut(&peer) {
                    tunnels.remove(&id);
                    if tunnels.is_empty() {
                        self.active.remove(&peer);
                    }
                }
            }
        }
    }

    pub fn get(&self, peer: &SpkiHash) -> Option<&HashMap<Uuid, TunnelDefinition>> {
        self.active.get(peer)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&SpkiHash, &HashMap<Uuid, TunnelDefinition>)> {
        self.active.iter()
    }
}
