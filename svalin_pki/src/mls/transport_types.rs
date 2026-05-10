use std::fmt::Debug;

use openmls::prelude::{
    MlsMessageBodyIn, MlsMessageBodyOut, MlsMessageIn, MlsMessageOut, PrivateMessageIn,
    ProtocolMessage, PublicMessageIn, Welcome,
    group_info::{GroupInfo, VerifiableGroupInfo},
};
use tls_codec::{DeserializeBytes, Serialize};

use crate::SpkiHash;

#[derive(serde::Serialize, serde::Deserialize)]
pub enum MessageToServerTransport {
    GroupMessage(Vec<u8>),
    NewDeviceGroup(NewGroupTransport),
    AddToGroup(AddToGroupTransport),
}

impl Debug for MessageToServerTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GroupMessage(_) => f.debug_tuple("GroupMessage").finish(),
            Self::NewDeviceGroup(new_group) => {
                f.debug_tuple("NewDeviceGroup").field(new_group).finish()
            }
            Self::AddToGroup(_) => f.debug_tuple("AddToGroup").finish(),
        }
    }
}

pub(crate) enum MessageToServer {
    GroupMessage {
        raw: Vec<u8>,
        message: ProtocolMessage,
    },
    NewDeviceGroup(NewGroup),
    AddToGroup(AddToGroup),
}

impl MessageToServerTransport {
    pub(crate) fn unpack(self) -> anyhow::Result<MessageToServer> {
        let to_server = match self {
            Self::GroupMessage(raw) => {
                let message =
                    MlsMessageIn::tls_deserialize_exact_bytes(&raw)?.try_into_protocol_message()?;
                MessageToServer::GroupMessage { raw, message }
            }
            Self::NewDeviceGroup(device_group) => {
                MessageToServer::NewDeviceGroup(device_group.unpack()?)
            }
            Self::AddToGroup(add_to_group) => MessageToServer::AddToGroup(add_to_group.unpack()?),
        };

        Ok(to_server)
    }

    pub fn message(mls_message: MlsMessageOut) -> Result<Self, tls_codec::Error> {
        let MlsMessageBodyOut::PrivateMessage(_) = mls_message.body() else {
            return Err(tls_codec::Error::DecodingError(
                "Expected a Private MLS message, but got something else".into(),
            ));
        };

        Ok(Self::GroupMessage(mls_message.tls_serialize_detached()?))
    }

    pub fn new_device_group(
        group_info: &GroupInfo,
        welcome: Option<&Welcome>,
    ) -> Result<Self, tls_codec::Error> {
        let welcome = if let Some(welcome) = welcome {
            Some(welcome.tls_serialize_detached()?)
        } else {
            None
        };

        Ok(Self::NewDeviceGroup(NewGroupTransport {
            group_info: group_info.tls_serialize_detached()?,
            welcome: welcome,
        }))
    }

    pub(crate) fn add_to_group(
        commit: MlsMessageOut,
        welcome: MlsMessageOut,
    ) -> Result<Self, tls_codec::Error> {
        let MlsMessageBodyOut::PublicMessage(_) = commit.body() else {
            return Err(tls_codec::Error::DecodingError(
                "Expected a Public MLS message, but got something else".into(),
            ));
        };
        let MlsMessageBodyOut::Welcome(welcome) = welcome.body() else {
            return Err(tls_codec::Error::DecodingError(
                "Expected a Welcome message, but got something else".into(),
            ));
        };

        Ok(Self::AddToGroup(AddToGroupTransport {
            commit: commit.tls_serialize_detached()?,
            welcome: welcome.tls_serialize_detached()?,
        }))
    }

    #[cfg(test)]
    #[allow(private_interfaces)]
    pub fn to_member(self) -> Result<MessageToMember, tls_codec::Error> {
        let transport = match self {
            Self::GroupMessage(message) => MessageToMemberTransport::GroupMessage(message),
            Self::NewDeviceGroup(device_group) => {
                MessageToMemberTransport::Welcome(device_group.welcome.unwrap())
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

impl MessageToSend {
    pub fn remove_receiver(&mut self, receiver: &SpkiHash) {
        self.receivers.retain(|r| r != receiver);
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
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
                let welcome = Welcome::tls_deserialize_exact_bytes(&welcome)?;
                // let message = MlsMessageIn::tls_deserialize_exact_bytes(&welcome)?;
                // let MlsMessageBodyIn::Welcome(welcome) = message.extract() else {
                //     return Err(tls_codec::Error::DecodingError(
                //         "Expected a welcome message, but got something else".into(),
                //     ));
                // };
                MessageToMember::Welcome(welcome)
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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct NewGroupTransport {
    group_info: Vec<u8>,
    welcome: Option<Vec<u8>>,
}

impl Debug for NewGroupTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewGroupTransport")
            .field("welcome", &self.welcome.as_ref().map(|_| ()))
            .finish()
    }
}

impl NewGroupTransport {
    pub(crate) fn unpack(self) -> Result<NewGroup, tls_codec::Error> {
        Ok(NewGroup {
            group_info: VerifiableGroupInfo::tls_deserialize_exact_bytes(&self.group_info)?,
            welcome: self.welcome,
        })
    }

    #[cfg(test)]
    #[allow(private_interfaces)]
    pub fn to_member(self) -> Result<MessageToMember, tls_codec::Error> {
        let transport = MessageToMemberTransport::Welcome(self.welcome.unwrap());

        transport.unpack()
    }
}

#[derive(Clone)]
pub struct NewGroup {
    pub(crate) group_info: VerifiableGroupInfo,
    pub(crate) welcome: Option<Vec<u8>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum DeviceMessage<Report> {
    Report(Report),
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AddToGroupTransport {
    commit: Vec<u8>,
    welcome: Vec<u8>,
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
