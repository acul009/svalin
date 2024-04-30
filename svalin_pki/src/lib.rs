mod certificate;
mod certificate_request;
mod encrypt;
mod error;
mod hash;
mod keypair;
mod perm_credentials;
mod public_key;
mod signed_message;

use anyhow::Result;
pub use certificate::Certificate;
pub use certificate_request::CertificateRequest;
pub use error::Error;
pub use hash::*;
pub use keypair::Keypair;
pub use perm_credentials::PermCredentials;

trait Credentials {
    fn sign(&self, data: &[u8]) -> Result<&[u8]>;
    fn verify(&self, crypto_data: &[u8]) -> Result<bool>;
}
