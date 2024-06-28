use std::sync::Arc;

use anyhow::Result;

use crate::server::agent_store::AgentStore;

struct AddAgentHandler {
    store: Arc<AgentStore>,
}

impl AddAgentHandler {
    pub fn new(store: Arc<AgentStore>) -> Result<Self> {
        Ok(Self { store })
    }
}
