use serde::{Deserialize, Serialize};
use svalin_pki::mls::{
    key_package::UnverifiedKeyPackage,
    transport_types::{MessageToMemberTransport, MessageToServerTransport},
};

pub mod agent;
pub mod server;

#[derive(Serialize, Deserialize)]
pub enum MessageToAgent {
    Mls(MessageToMemberTransport),
    KeyPackageCount(u64),
}

#[derive(Serialize, Deserialize)]
pub enum MessageFromAgent {
    Mls(MlsToServer),
}

#[derive(Serialize, Deserialize)]
pub enum MlsToServer {
    Mls(MessageToServerTransport),
    KeyPackage(UnverifiedKeyPackage),
}
