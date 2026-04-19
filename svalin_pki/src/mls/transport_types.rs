use std::fmt::Debug;

use openmls::prelude::{
    MlsMessageBodyIn, MlsMessageIn, PrivateMessageIn, Welcome, group_info::VerifiableGroupInfo,
};
use serde::{Deserialize, Serialize};
use tls_codec::DeserializeBytes;

use crate::SpkiHash;

#[derive(Serialize, Deserialize)]
pub enum MessageToServerTransport {
    GroupMessage(Vec<u8>),
    NewDeviceGroup { device_group: NewGroupTransport },
}

impl Debug for MessageToServerTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GroupMessage(_) => f.debug_tuple("GroupMessage").finish(),
            Self::NewDeviceGroup { .. } => f.debug_tuple("NewDeviceGroup").finish(),
        }
    }
}

impl MessageToServerTransport {
    pub(crate) fn unpack(self) -> Result<MessageToServer, tls_codec::Error> {
        let to_server = match self {
            Self::GroupMessage(message) => MessageToServer::GroupMessage(message),
            Self::NewDeviceGroup { device_group } => MessageToServer::NewDeviceGroup {
                device_group: device_group.unpack()?,
            },
        };

        Ok(to_server)
    }
}

pub(crate) enum MessageToServer {
    GroupMessage(Vec<u8>),
    NewDeviceGroup { device_group: NewGroup },
}

impl MessageToServerTransport {
    #[cfg(test)]
    pub fn to_member(self) -> Result<MessageToMember, tls_codec::Error> {
        let transport = match self {
            Self::GroupMessage(message) => MessageToMemberTransport::GroupMessage(message),
            Self::NewDeviceGroup { device_group } => {
                MessageToMemberTransport::Welcome(device_group.welcome)
            }
        };

        transport.unpack()
    }
}

#[derive(Debug)]
pub struct MessageToSend {
    pub receivers: Vec<SpkiHash>,
    pub message: MessageToMemberTransport,
}

#[derive(Serialize, Deserialize)]
pub enum MessageToMemberTransport {
    Welcome(Vec<u8>),
    GroupMessage(Vec<u8>),
}

impl Debug for MessageToMemberTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageToMemberTransport::Welcome(_) => f.debug_tuple("Welcome").finish(),
            MessageToMemberTransport::GroupMessage(_) => f.debug_tuple("GroupMessage").finish(),
        }
    }
}

impl MessageToMemberTransport {
    pub(crate) fn unpack(&self) -> Result<MessageToMember, tls_codec::Error> {
        let unpacked = match self {
            MessageToMemberTransport::Welcome(welcome) => {
                MessageToMember::Welcome(Welcome::tls_deserialize_exact_bytes(&welcome)?)
            }
            MessageToMemberTransport::GroupMessage(message) => {
                let message = MlsMessageIn::tls_deserialize_exact_bytes(&message)?;
                let MlsMessageBodyIn::PrivateMessage(private_message) = message.extract() else {
                    return Err(tls_codec::Error::DecodingError(
                        "Expected a Private MLS message, but got something else".into(),
                    ));
                };
                MessageToMember::GroupMessage(private_message)
            }
        };

        Ok(unpacked)
    }
}

pub(crate) enum MessageToMember {
    Welcome(Welcome),
    GroupMessage(PrivateMessageIn),
}

#[derive(Serialize, Deserialize)]
pub struct NewGroupTransport {
    pub(crate) group_info: Vec<u8>,
    pub(crate) welcome: Vec<u8>,
}

impl NewGroupTransport {
    pub(crate) fn unpack(self) -> Result<NewGroup, tls_codec::Error> {
        Ok(NewGroup {
            group_info: VerifiableGroupInfo::tls_deserialize_exact_bytes(&self.group_info)?,
            welcome: self.welcome,
        })
    }

    #[cfg(test)]
    pub fn to_member(self) -> Result<MessageToMember, tls_codec::Error> {
        let transport = MessageToMemberTransport::Welcome(self.welcome);

        transport.unpack()
    }
}

#[derive(Clone)]
pub struct NewGroup {
    pub(crate) group_info: VerifiableGroupInfo,
    pub(crate) welcome: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum DeviceMessage<Report> {
    Report(Report),
}
