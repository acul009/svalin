use openmls::prelude::{
    PrivateMessageIn, ProtocolMessage, Welcome, group_info::VerifiableGroupInfo,
};
use serde::{Deserialize, Serialize};
use tls_codec::DeserializeBytes;

use crate::SpkiHash;

#[derive(Serialize, Deserialize)]
pub enum MessageToServerTransport {
    NewGroup {
        group_info: Vec<u8>,
        welcome: Vec<u8>,
    },
    GroupMessage(Vec<u8>),
}

impl MessageToServerTransport {
    pub fn unpack(self) -> Result<MessageToServer, tls_codec::Error> {
        let unpacked = match self {
            MessageToServerTransport::NewGroup {
                group_info,
                welcome,
            } => MessageToServer::NewGroup {
                group_info: VerifiableGroupInfo::tls_deserialize_exact_bytes(&group_info)?,
                welcome,
            },
            MessageToServerTransport::GroupMessage(message) => {
                let private_message = PrivateMessageIn::tls_deserialize_exact_bytes(&message)?;
                let protocol: ProtocolMessage = private_message.into();
                let group_id = protocol.group_id().clone();
                MessageToServer::GroupMessage { group_id, message }
            }
        };

        Ok(unpacked)
    }

    #[cfg(test)]
    pub fn to_member(self) -> Result<MessageToMember, tls_codec::Error> {
        let transport = match self {
            MessageToServerTransport::NewGroup { welcome, .. } => {
                MessageToMemberTransport::Welcome(welcome)
            }
            MessageToServerTransport::GroupMessage(message) => {
                MessageToMemberTransport::GroupMessage(message)
            }
        };

        transport.unpack()
    }
}

pub enum MessageToServer {
    NewGroup {
        group_info: VerifiableGroupInfo,
        welcome: Vec<u8>,
    },
    GroupMessage {
        group_id: openmls::prelude::GroupId,
        message: Vec<u8>,
    },
}

pub struct NewGroupInfo {}

pub struct MessageToSend {
    pub receivers: Vec<SpkiHash>,
    pub message: MessageToMemberTransport,
}

#[derive(Serialize, Deserialize)]
pub enum MessageToMemberTransport {
    Welcome(Vec<u8>),
    GroupMessage(Vec<u8>),
}

impl MessageToMemberTransport {
    pub fn unpack(self) -> Result<MessageToMember, tls_codec::Error> {
        let unpacked = match self {
            MessageToMemberTransport::Welcome(welcome) => {
                MessageToMember::Welcome(Welcome::tls_deserialize_exact_bytes(&welcome)?)
            }
            MessageToMemberTransport::GroupMessage(message) => {
                let private_message = PrivateMessageIn::tls_deserialize_exact_bytes(&message)?;
                MessageToMember::GroupMessage(private_message)
            }
        };

        Ok(unpacked)
    }
}

pub enum MessageToMember {
    Welcome(Welcome),
    GroupMessage(PrivateMessageIn),
}
