#![forbid(unsafe_code)]
mod argon;
mod certificate;
mod certificate_chain;
mod credential;
mod encrypt;
mod keypair;
pub mod mls;
pub mod serde_paramsstring;
pub mod serde_saltstring;
mod signed_message;
mod signed_object;
#[cfg(test)]
mod test;
mod verifier;

// pub mod tbrhl;

// Re-Exports
pub use curve25519_dalek;
pub use password_hash;
pub use sha2::Sha512;

// Exports
pub use argon::{ArgonCost, ArgonParams, DeriveKeyError, ParamsStringParseError, PasswordHash};
pub use argon2;
pub use certificate::{
    Certificate, CertificateParseError, CertificateType, RootCertificate,
    SignatureVerificationError, SpkiHash, UnverifiedCertificate, UseAsRootError, ValidityError,
};
pub use certificate_chain::{
    AddCertificateError, CertificateChain, CertificateChainBuilder, UnverifiedCertificateChain,
    VerifyChainError,
};
pub use credential::{
    CreateCertificateError, CreateCredentialsError, Credential, DecodeCredentialsError,
    EncryptedCredential,
};
pub use encrypt::{DecryptError, EncryptError, EncryptedData, EncryptedObject};
pub use keypair::{ExportedPublicKey, KeyPair};
// pub use signed_object::{SignedObject, VerifiedObject};
pub use signed_object::{SignedObject, VerifiedObject};
pub use verifier::{KnownCertificateVerifier, Verifier, VerifyError, exact::ExactVerififier};

// normal use statements
use anyhow::Result;
use ring::rand::{SecureRandom, SystemRandom};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

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
