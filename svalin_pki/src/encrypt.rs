use std::{fmt::Debug, marker::PhantomData};

use anyhow::Result;

use ring::aead::{
    AES_256_GCM, Aad, BoundKey, CHACHA20_POLY1305, NONCE_LEN, Nonce, NonceSequence, OpeningKey,
    SealingKey, UnboundKey,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
enum EncryptionAlgorithm {
    Chacha20Poly1305,
    Aes256Gcm,
}

impl From<EncryptionAlgorithm> for &'static ring::aead::Algorithm {
    fn from(value: EncryptionAlgorithm) -> Self {
        match value {
            EncryptionAlgorithm::Chacha20Poly1305 => &CHACHA20_POLY1305,
            EncryptionAlgorithm::Aes256Gcm => &AES_256_GCM,
        }
    }
}

pub struct EncryptionKey([u8; 32]);

impl EncryptionKey {
    pub fn dangerous_from_bytes(key: [u8; 32]) -> Self {
        Self(key)
    }
}

impl AsRef<[u8; 32]> for EncryptionKey {
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedData {
    ciphertext: Vec<u8>,
    alg: EncryptionAlgorithm,
}

#[derive(Serialize, Deserialize)]
pub struct EncryptedObject<T> {
    phantom: PhantomData<T>,
    ciphertext: EncryptedData,
}

impl<T> Debug for EncryptedObject<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptedObject")
            .field("ciphertext", &self.ciphertext)
            .finish()
    }
}

impl<T> Clone for EncryptedObject<T> {
    fn clone(&self) -> Self {
        Self {
            phantom: PhantomData,
            ciphertext: self.ciphertext.clone(),
        }
    }
}

impl<T> EncryptedObject<T>
where
    T: Serialize,
{
    pub fn encrypt(object: &T, encryption_key: &EncryptionKey) -> Result<Self, EncryptError> {
        let serialized = postcard::to_stdvec(object)?;
        let ciphertext = EncryptedData::encrypt(&serialized, encryption_key)?;
        Ok(Self {
            ciphertext,
            phantom: PhantomData,
        })
    }
}

impl<T> EncryptedObject<T>
where
    T: DeserializeOwned,
{
    pub fn decrypt(self, encryption_key: &EncryptionKey) -> Result<T, DecryptError> {
        let encoded = self.ciphertext.decrypt(encryption_key)?;
        Ok(postcard::from_bytes(&encoded)?)
    }
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

static DEFAULT_ALG: EncryptionAlgorithm = EncryptionAlgorithm::Chacha20Poly1305;

#[derive(Debug, thiserror::Error)]
pub enum DecryptError {
    #[error("error decoding encrypted data: {0}")]
    UnmarshalError(#[from] postcard::Error),
    #[error("missing hash parameters in encrypted data")]
    MissingHashParameters,
    #[error("error loading key into ring: {0}")]
    CreateUnboundError(ring::error::Unspecified),
    #[error("error decrypting data: {0}")]
    UnsealError(ring::error::Unspecified),
}

#[derive(Debug, thiserror::Error)]
pub enum EncryptError {
    #[error("error marshalling data: {0}")]
    MarshalError(#[from] postcard::Error),
    #[error("error loading key into ring: {0}")]
    CreateUnboundError(ring::error::Unspecified),
    #[error("error sealing data: {0}")]
    SealError(ring::error::Unspecified),
}

impl EncryptedData {
    pub fn encrypt(data: &[u8], encryption_key: &EncryptionKey) -> Result<Self, EncryptError> {
        Self::encrypt_with_alg(data, encryption_key, DEFAULT_ALG)
    }

    fn encrypt_with_alg(
        data: &[u8],
        encryption_key: &EncryptionKey,
        alg: EncryptionAlgorithm,
    ) -> Result<Self, EncryptError> {
        let ring_alg = alg.into();

        let unbound = UnboundKey::new(ring_alg, encryption_key.0.as_ref())
            .map_err(EncryptError::CreateUnboundError)?;
        let mut sealing = SealingKey::new(unbound, NonceCounter::new());

        let mut encrypted = data.to_owned();

        sealing
            .seal_in_place_append_tag(Aad::empty(), &mut encrypted)
            .map_err(EncryptError::SealError)?;

        Ok(Self {
            alg,
            ciphertext: encrypted,
        })
    }

    pub fn decrypt(mut self, encryption_key: &EncryptionKey) -> Result<Vec<u8>, DecryptError> {
        let ring_alg = self.alg.into();

        let unbound = UnboundKey::new(ring_alg, encryption_key.0.as_ref())
            .map_err(DecryptError::CreateUnboundError)?;
        let mut opening = OpeningKey::new(unbound, NonceCounter::new());

        let cleartext_len = opening
            .open_in_place(Aad::empty(), &mut self.ciphertext)
            .map_err(DecryptError::UnsealError)?
            .len();

        self.ciphertext.drain(cleartext_len..);

        Ok(self.ciphertext)
    }
}

#[cfg(test)]
mod test {
    use ring::rand::{SecureRandom, SystemRandom};

    use crate::generate_key;

    use super::EncryptedData;

    #[tokio::test]
    async fn encrypt_decrypt() {
        let rand = SystemRandom::new();
        let mut msg = [0u8; 1024];
        rand.fill(&mut msg).unwrap();

        let key = generate_key().unwrap();

        let encrypted = EncryptedData::encrypt(&msg, &key).unwrap();
        let msg2 = encrypted.decrypt(&key).unwrap();

        assert_eq!(msg.as_ref(), &msg2);
    }
}
