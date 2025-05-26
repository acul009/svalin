use anyhow::Result;
use serde::{Deserialize, Serialize};
use svalin_pki::{EncryptedCredentials, PermCredentials};
use tracing::debug;

/// The keysource enum is saved in the configuration and specifies how to
/// load the key for decrypting the credentials. This will enable the use of
/// external key management systems should that be necessary one day
#[derive(Serialize, Deserialize)]
pub enum KeySource {
    BuiltIn([u8; 32]),
}

impl KeySource {
    async fn to_key(&self) -> Result<Vec<u8>> {
        match self {
            KeySource::BuiltIn(k) => Ok(k.to_vec()),
        }
    }

    pub fn generate_builtin() -> Result<Self> {
        let key = svalin_pki::generate_key()?;
        Ok(Self::BuiltIn(key))
    }

    pub async fn encrypt_credentials(
        &self,
        credentials: &PermCredentials,
    ) -> Result<EncryptedCredentials> {
        let key = self.to_key().await?;

        credentials.export(key).await
    }

    pub async fn decrypt_credentials(
        &self,
        encrypted_credentials: EncryptedCredentials,
    ) -> Result<PermCredentials> {
        let key = self.to_key().await?;
        debug!("headless password loaded, decrypting...");
        Ok(encrypted_credentials.decrypt(key).await?)
    }
}
