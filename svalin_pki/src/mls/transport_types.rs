use std::fmt::Debug;

use openmls::prelude::{
    MlsMessageBodyIn, MlsMessageIn, PrivateMessageIn, PublicMessageIn, Welcome,
    group_info::VerifiableGroupInfo,
};
use serde::{Deserialize, Serialize};
use tls_codec::DeserializeBytes;

use crate::SpkiHash;

#[derive(Serialize, Deserialize)]
pub enum MessageToServerTransport {
    GroupMessage(Vec<u8>),
    NewDeviceGroup { device_group: NewGroupTransport },
    AddToGroup(AddToGroupTransport),
}

impl Debug for MessageToServerTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GroupMessage(_) => f.debug_tuple("GroupMessage").finish(),
            Self::NewDeviceGroup { .. } => f.debug_tuple("NewDeviceGroup").finish(),
            Self::AddToGroup(_) => f.debug_tuple("AddToGroup").finish(),
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
            Self::AddToGroup(add_to_group) => MessageToServer::AddToGroup(add_to_group.unpack()?),
        };

        Ok(to_server)
    }
}

pub(crate) enum MessageToServer {
    GroupMessage(Vec<u8>),
    NewDeviceGroup { device_group: NewGroup },
    AddToGroup(AddToGroup),
}

impl MessageToServerTransport {
    #[cfg(test)]
    pub fn to_member(self) -> Result<MessageToMember, tls_codec::Error> {
        let transport = match self {
            Self::GroupMessage(message) => MessageToMemberTransport::GroupMessage(message),
            Self::NewDeviceGroup { device_group } => {
                MessageToMemberTransport::Welcome(device_group.welcome)
            }
            Self::AddToGroup(add_to_group) => {
                MessageToMemberTransport::AddToGroup(add_to_group.commit)
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
    AddToGroup(Vec<u8>),
}

impl Debug for MessageToMemberTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageToMemberTransport::Welcome(_) => f.debug_tuple("Welcome").finish(),
            MessageToMemberTransport::GroupMessage(_) => f.debug_tuple("GroupMessage").finish(),
            MessageToMemberTransport::AddToGroup(_) => f.debug_tuple("AddToGroup").finish(),
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
            MessageToMemberTransport::AddToGroup(commit) => {
                let commit = MlsMessageIn::tls_deserialize_exact_bytes(&commit)?;
                let MlsMessageBodyIn::PublicMessage(commit) = commit.extract() else {
                    return Err(tls_codec::Error::DecodingError(
                        "Expected a Private MLS message, but got something else".into(),
                    ));
                };
                MessageToMember::AddToGroup(commit)
            }
        };

        Ok(unpacked)
    }
}

pub(crate) enum MessageToMember {
    Welcome(Welcome),
    GroupMessage(PrivateMessageIn),
    AddToGroup(PublicMessageIn),
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

#[derive(Serialize, Deserialize)]
pub struct AddToGroupTransport {
    pub(crate) commit: Vec<u8>,
    pub(crate) welcome: Vec<u8>,
}

impl AddToGroupTransport {
    fn unpack(&self) -> Result<AddToGroup, tls_codec::Error> {
        let commit = MlsMessageIn::tls_deserialize_exact_bytes(&self.commit)?;
        let MlsMessageBodyIn::PublicMessage(commit) = commit.extract() else {
            return Err(tls_codec::Error::DecodingError(
                "Expected a Private MLS message, but got something else".into(),
            ));
        };

        Ok(AddToGroup {
            commit,
            commit_bytes: self.commit.clone(),
            welcome: self.welcome.clone(),
        })
    }
}

pub struct AddToGroup {
    pub(crate) commit: PublicMessageIn,
    pub(crate) commit_bytes: Vec<u8>,
    pub(crate) welcome: Vec<u8>,
}
