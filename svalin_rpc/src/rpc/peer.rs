use std::fmt::Debug;

use svalin_pki::Certificate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Peer {
    Anonymous,
    Certificate(Certificate),
}

impl Peer {
    pub fn certificate(&self) -> Result<&Certificate, WrongPeerTypeError> {
        match self {
            Peer::Anonymous => Err(WrongPeerTypeError::ExpectedCertificate),
            Peer::Certificate(cert) => Ok(cert),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WrongPeerTypeError {
    #[error("expected peer of type certificate")]
    ExpectedCertificate,
}
