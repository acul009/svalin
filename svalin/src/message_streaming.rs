use std::sync::Arc;

use serde::{Deserialize, Serialize};
use svalin_pki::{
    SpkiHash,
    mls::transport_types::{MessageToMemberTransport, MessageToServerTransport},
};

pub mod agent;
pub mod client;
pub mod server;
pub mod with_agent;
pub mod with_client;

#[derive(Serialize, Deserialize)]
pub enum MessageToAgent {
    Mls(MessageToMemberTransport),
    Goodbye,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MessageFromAgent {
    Mls(MessageToServerTransport),
    Goodbye,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MessageToClient {
    AgentOnlineStatus(SpkiHash, bool),
    // The arc is unfortunately needed currently, so the server doesn't have to copy as much data
    Mls(Arc<MessageToMemberTransport>),
    Goodbye,
}

#[derive(Serialize, Deserialize)]
pub enum MessageFromClient {
    Mls(MessageToServerTransport),
    Goodbye,
}
