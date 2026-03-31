use openmls::prelude::{
    MlsMessageBodyIn, MlsMessageIn, PrivateMessageIn, ProtocolMessage, Welcome,
    group_info::VerifiableGroupInfo,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tls_codec::DeserializeBytes;

use crate::{SpkiHash, UnverifiedCertificate};

#[derive(Serialize, Deserialize)]
pub enum MessageToServer {
    GroupMessage(Vec<u8>),
}

impl MessageToServer {
    #[cfg(test)]
    pub fn to_member(self) -> Result<MessageToMember, tls_codec::Error> {
        let transport = match self {
            Self::GroupMessage(message) => MessageToMemberTransport::GroupMessage(message),
        };

        transport.unpack()
    }
}

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
    pub(crate) fn unpack(self) -> Result<MessageToMember, tls_codec::Error> {
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

pub enum MessageToMember {
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

    pub fn extract_welcome(&self) -> MessageToMemberTransport {
        MessageToMemberTransport::Welcome(self.welcome.clone())
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
