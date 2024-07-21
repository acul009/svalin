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
pub mod tbrhl;

use anyhow::{anyhow, Result};
pub use certificate::Certificate;
pub use certificate_request::CertificateRequest;
pub use error::Error;
pub use hash::*;
pub use keypair::Keypair;
pub use perm_credentials::PermCredentials;
use ring::rand::{SecureRandom, SystemRandom};

pub fn generate_key() -> Result<[u8; 32]> {
    let rand = SystemRandom::new();

    let mut msg = [0u8; 32];
    rand.fill(&mut msg).map_err(|err| anyhow!(err))?;

    Ok(msg)
}
