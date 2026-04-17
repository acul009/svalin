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
}

#[derive(Serialize, Deserialize)]
pub enum MessageFromAgent {
    Mls(MessageToServerTransport),
}

#[derive(Serialize, Deserialize)]
pub enum MessageToClient {
    AgentOnlineStatus(SpkiHash, bool),
    Mls(MessageToMemberTransport),
}

#[derive(Serialize, Deserialize)]
pub enum MessageFromClient {
    Mls(MessageToServerTransport),
}
