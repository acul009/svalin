use anyhow::Result;
use serde::{Deserialize, Serialize};
use svalin_pki::{Credential, EncryptedCredential, EncryptionKey};

/// The keysource enum is saved in the configuration and specifies how to
/// load the key for decrypting the credentials. This will enable the use of
/// external key management systems should that be necessary one day
#[derive(Serialize, Deserialize)]
pub enum KeySource {
    BuiltIn([u8; 32]),
}

impl KeySource {
    async fn to_key(&self) -> Result<EncryptionKey> {
        match self {
            KeySource::BuiltIn(k) => Ok(EncryptionKey::dangerous_from_bytes(k.clone())),
        }
    }

    pub fn generate_builtin() -> Result<Self> {
        let key = svalin_pki::generate_key()?;
        Ok(Self::BuiltIn(key.as_ref().clone()))
    }

    pub async fn encrypt_credential(
        &self,
        credentials: &Credential,
    ) -> Result<EncryptedCredential> {
        let key = self.to_key().await?;

        Ok(credentials.export(&key)?)
    }

    pub async fn decrypt_credentials(
        &self,
        encrypted_credentials: EncryptedCredential,
    ) -> Result<Credential> {
        let key = self.to_key().await?;
        tracing::trace!("headless password loaded, decrypting...");
        Ok(encrypted_credentials.decrypt(&key)?)
    }
}
