use anyhow::{Context, Result, anyhow};

use ring::aead::{
    AES_256_GCM, Aad, BoundKey, CHACHA20_POLY1305, NONCE_LEN, Nonce, NonceSequence, OpeningKey,
    SealingKey, UnboundKey,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::debug;

use crate::hash::ArgonParams;

#[derive(Serialize, Deserialize, Clone, Copy)]
enum EncryptionAlgorythm {
    Chacha20Poly1305,
    Aes256Gcm,
}

impl From<EncryptionAlgorythm> for &'static ring::aead::Algorithm {
    fn from(value: EncryptionAlgorythm) -> Self {
        match value {
            EncryptionAlgorythm::Chacha20Poly1305 => &CHACHA20_POLY1305,
            EncryptionAlgorythm::Aes256Gcm => &AES_256_GCM,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct EncryptedData {
    parameters: Option<ArgonParams>,
    ciphertext: Vec<u8>,
    alg: EncryptionAlgorythm,
}

struct NonceCounter {
    counter: u64,
}

impl NonceCounter {
    fn new() -> Self {
        Self { counter: 1 }
    }
}

impl NonceSequence for NonceCounter {
    fn advance(&mut self) -> std::prelude::v1::Result<ring::aead::Nonce, ring::error::Unspecified> {
        let bytes = self.counter.to_be_bytes();
        self.counter += 1;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        nonce_bytes[..8].clone_from_slice(&bytes);

        Nonce::try_assume_unique_for_key(&nonce_bytes)
    }
}

static DEFAULT_ALG: EncryptionAlgorythm = EncryptionAlgorythm::Chacha20Poly1305;

impl EncryptedData {
    pub async fn encrypt_with_password(data: &[u8], password: Vec<u8>) -> Result<Vec<u8>> {
        let parameters = ArgonParams::strong();
        let encryption_key = parameters.derive_key(password).await?;
        EncryptedData::encrypt_with_alg(data, encryption_key, DEFAULT_ALG, Some(parameters))
    }

    pub fn encrypt_object_with_key<T: Serialize>(
        data: &T,
        encryption_key: [u8; 32],
    ) -> Result<Vec<u8>> {
        let serialized = postcard::to_extend(data, Vec::new())?;
        Self::encrypt_with_key(&serialized, encryption_key)
    }

    pub fn encrypt_with_key(data: &[u8], encryption_key: [u8; 32]) -> Result<Vec<u8>> {
        Self::encrypt_with_alg(data, encryption_key, DEFAULT_ALG, None)
    }

    fn encrypt_with_alg(
        data: &[u8],
        encryption_key: [u8; 32],
        alg: EncryptionAlgorythm,
        parameters: Option<ArgonParams>,
    ) -> Result<Vec<u8>> {
        let ring_alg = alg.into();

        let unbound = UnboundKey::new(ring_alg, &encryption_key).map_err(|err| anyhow!(err))?;
        let mut sealing = SealingKey::new(unbound, NonceCounter::new());

        let mut encrypted = data.to_owned();

        sealing
            .seal_in_place_append_tag(Aad::empty(), &mut encrypted)
            .map_err(|err| anyhow!(err))
            .context("failed ring sealing in place")?;

        let encrypted_data = EncryptedData {
            alg,
            parameters,
            ciphertext: encrypted,
        };

        let packed = Vec::new();
        postcard::to_extend(&encrypted_data, packed)
            .map_err(|err| anyhow!(err))
            .context("failed postcard encoding")
    }

    pub async fn decrypt_with_password(cipherdata: &[u8], password: Vec<u8>) -> Result<Vec<u8>> {
        let encrypted_data: EncryptedData = postcard::from_bytes(cipherdata)?;

        let parameters = if let Some(parameters) = &encrypted_data.parameters {
            parameters
        } else {
            return Err(anyhow!("encrypted data has no hash parameters"));
        };

        debug!("deriving key");
        let encryption_key = parameters.derive_key(password).await?;

        Self::decrypt_helper(encrypted_data, encryption_key)
    }

    pub fn decrypt_with_key(cipherdata: &[u8], encryption_key: [u8; 32]) -> Result<Vec<u8>> {
        let encrypted_data: EncryptedData = postcard::from_bytes(cipherdata)?;

        Self::decrypt_helper(encrypted_data, encryption_key)
    }

    pub fn decrypt_object_with_key<T: DeserializeOwned>(
        cipherdata: &[u8],
        encryption_key: [u8; 32],
    ) -> Result<T> {
        let decrypted_data = Self::decrypt_with_key(cipherdata, encryption_key)?;

        postcard::from_bytes(&decrypted_data)
            .map_err(|err| anyhow!(err))
            .context("failed postcard decoding")
    }

    fn decrypt_helper(
        mut encrypted_data: EncryptedData,
        encryption_key: [u8; 32],
    ) -> Result<Vec<u8>> {
        let ring_alg = encrypted_data.alg.into();

        let unbound = UnboundKey::new(ring_alg, &encryption_key).map_err(|err| anyhow!(err))?;
        let mut opening = OpeningKey::new(unbound, NonceCounter::new());

        let cleartext_len = opening
            .open_in_place(Aad::empty(), &mut encrypted_data.ciphertext)
            .map_err(|err| anyhow!(err))
            .context("failed ring unsealing")?
            .len();

        encrypted_data.ciphertext.drain(cleartext_len..);

        Ok(encrypted_data.ciphertext)
    }
}

#[cfg(test)]
mod test {
    use ring::rand::{SecureRandom, SystemRandom};

    use super::EncryptedData;

    #[tokio::test]
    async fn encrypt_decrypt() {
        let rand = SystemRandom::new();
        let mut msg = [0u8; 1024];
        rand.fill(&mut msg).unwrap();

        let password = "testpass".as_bytes();

        let encrypted = EncryptedData::encrypt_with_password(&msg, password.to_owned())
            .await
            .unwrap();
        let msg2 = EncryptedData::decrypt_with_password(&encrypted, password.to_owned())
            .await
            .unwrap();

        assert_eq!(msg.as_ref(), &msg2);
    }
}
