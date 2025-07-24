mod argon;
mod certificate;
mod certificate_request;
mod credential;
mod encrypt;
mod keypair;
mod public_key;
mod signed_message;
mod signed_object;
// pub mod tbrhl;
mod verifier;

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
pub use argon::{ArgonCost, ArgonParams, DeriveKeyError, ParamsStringParseError, PasswordHash};
pub use certificate::{
    Certificate, CertificateParseError, SignatureVerificationError, ValidityError,
};
pub use certificate_request::{CertificateRequest, CertificateRequestParseError};
pub use credential::{
    ApproveRequestError, CreateCredentialsError, Credential, DecodeCredentialsError,
    EncryptedCredentials,
};
pub use encrypt::{DecryptError, EncryptError, EncryptedData, EncryptedObject};
pub use keypair::{GenerateRequestError, KeyPair, ToSelfSingedError};
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
