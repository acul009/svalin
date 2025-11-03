use openmls::prelude::{MlsMessageIn, MlsMessageOut, Welcome};
use tls_codec::{DeserializeBytes, Serialize};

use crate::Certificate;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Invitation {
    receivers: Vec<Certificate>,
    invitation: Vec<u8>,
}

#[derive(thiserror::Error, Debug)]
pub enum InvitationError {
    #[error("TLS codec error: {0}")]
    TlsCodec(#[from] tls_codec::Error),
    #[error("Not an invitation")]
    NotAnInvitation,
}

impl Invitation {
    pub(crate) fn new(
        invitation: MlsMessageOut,
        receivers: Vec<Certificate>,
    ) -> Result<Self, InvitationError> {
        match invitation.body() {
            openmls::prelude::MlsMessageBodyOut::Welcome(_) => {}
            _ => return Err(InvitationError::NotAnInvitation),
        }

        Ok(Self {
            receivers,
            invitation: invitation.tls_serialize_detached()?,
        })
    }

    pub fn get_invitation(&self) -> Result<Welcome, InvitationError> {
        let invitation = MlsMessageIn::tls_deserialize_exact_bytes(&self.invitation)?;
        match invitation.extract() {
            openmls::prelude::MlsMessageBodyIn::Welcome(welcome) => Ok(welcome),
            _ => Err(InvitationError::NotAnInvitation),
        }
    }
}
