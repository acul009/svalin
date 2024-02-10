mod certificate;
mod encrypt;
mod error;
mod hash;
mod perm_credentials;
mod signed_message;
mod temp_credentials;

use anyhow::Result;
pub use certificate::Certificate;
pub use error::Error;
pub use hash::ArgonParams;
pub use perm_credentials::PermCredentials;
pub use temp_credentials::TempCredentials;

trait Credentials {
    fn sign(&self, data: &[u8]) -> Result<&[u8]>;
    fn verify(&self, crypto_data: &[u8]) -> Result<bool>;
}
