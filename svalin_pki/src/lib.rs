mod certificate;
mod certificate_request;
mod encrypt;
mod error;
mod hash;
mod keypair;
mod perm_credentials;
mod public_key;
// pub mod sealed_object;
pub mod hybrid_encrypted_object;
mod signed_message;
pub mod signed_object;
// pub mod tbrhl;
pub mod verifier;

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
pub use certificate::Certificate;
pub use certificate::CertificateParseError;
pub use certificate_request::{CertificateRequest, CertificateRequestParseError};
pub use encrypt::{DecryptError, EncryptError, EncryptedData, EncryptedObject};
pub use error::Error;
pub use hash::*;
pub use keypair::{GenerateRequestError, Keypair, ToSelfSingedError};
pub use perm_credentials::{
    ApproveRequestError, CreateCredentialsError, DecodeCredentialsError, EncryptedCredentials,
    PermCredentials,
};
use ring::rand::{SecureRandom, SystemRandom};

pub use argon2;
pub use sha2;
use thiserror::Error;

#[cfg(test)]
mod test;

#[derive(Error, Debug)]
pub enum GenerateKeyError {
    #[error("Unspecified error")]
    UnspecifiedError(ring::error::Unspecified),
}

pub fn generate_key() -> Result<[u8; 32], GenerateKeyError> {
    let rand = SystemRandom::new();

    let mut msg = [0u8; 32];
    rand.fill(&mut msg)
        .map_err(GenerateKeyError::UnspecifiedError)?;

    Ok(msg)
}

pub fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
