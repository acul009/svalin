mod argon;
mod certificate;
mod credential;
mod encrypt;
mod keypair;
mod mls;
mod public_key;
mod signed_message;
mod signed_object;
mod verifier;

// pub mod tbrhl;

// Re-Exports
pub use argon::{ArgonCost, ArgonParams, DeriveKeyError, ParamsStringParseError, PasswordHash};
pub use argon2;
pub use certificate::{
    Certificate, CertificateParseError, SignatureVerificationError, ValidityError,
};
pub use credential::{
    ApproveRequestError, CreateCredentialsError, Credential, DecodeCredentialsError,
    EncryptedCredentials,
};
pub use encrypt::{DecryptError, EncryptError, EncryptedData, EncryptedObject};
pub use keypair::{ExportedPublicKey, KeyPair};
// pub use signed_object::{SignedObject, VerifiedObject};
pub use verifier::{KnownCertificateVerifier, VerificationError, Verifier};

// normal use statements
use anyhow::Result;
use ring::rand::{SecureRandom, SystemRandom};
use std::time::{SystemTime, UNIX_EPOCH};

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
