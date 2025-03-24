mod certificate;
mod certificate_request;
mod encrypt;
mod error;
mod hash;
mod keypair;
mod perm_credentials;
mod public_key;
mod signed_message;
pub mod signed_object;
// pub mod tbrhl;
pub mod verifier;

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
pub use certificate::Certificate;
pub use certificate::CertificateParseError;
pub use certificate_request::CertificateRequest;
pub use encrypt::EncryptedData;
pub use error::Error;
pub use hash::*;
pub use keypair::Keypair;
pub use perm_credentials::PermCredentials;
use ring::rand::{SecureRandom, SystemRandom};

pub use argon2;
pub use sha2;

#[cfg(test)]
mod test;

pub fn generate_key() -> Result<[u8; 32]> {
    let rand = SystemRandom::new();

    let mut msg = [0u8; 32];
    rand.fill(&mut msg).map_err(|err| anyhow!(err))?;

    Ok(msg)
}

pub fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
