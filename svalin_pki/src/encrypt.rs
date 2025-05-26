use std::{fmt::Debug, marker::PhantomData};

use anyhow::Result;

use ring::aead::{
    AES_256_GCM, Aad, BoundKey, CHACHA20_POLY1305, NONCE_LEN, Nonce, NonceSequence, OpeningKey,
    SealingKey, UnboundKey,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::hash::ArgonParams;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedData {
    parameters: Option<ArgonParams>,
    ciphertext: Vec<u8>,
    alg: EncryptionAlgorythm,
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
    pub fn encrypt_with_key(object: &T, encryption_key: [u8; 32]) -> Result<Self, EncryptError> {
        let serialized = postcard::to_stdvec(object)?;
        let ciphertext = EncryptedData::encrypt_with_key(&serialized, encryption_key)?;
        Ok(Self {
            ciphertext,
            phantom: PhantomData,
        })
    }

    pub async fn encrypt_with_password(
        object: &T,
        password: Vec<u8>,
    ) -> Result<Self, EncryptError> {
        let serialized = postcard::to_stdvec(object)?;
        let ciphertext = EncryptedData::encrypt_with_password(&serialized, password).await?;
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
    pub fn decrypt_with_key(self, encryption_key: [u8; 32]) -> Result<T, DecryptError> {
        let encoded = self.ciphertext.decrypt_with_key(encryption_key)?;
        Ok(postcard::from_bytes(&encoded)?)
    }

    pub async fn decrypt_with_password(self, password: Vec<u8>) -> Result<T, DecryptError> {
        let encoded = self.ciphertext.decrypt_with_password(password).await?;
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

static DEFAULT_ALG: EncryptionAlgorythm = EncryptionAlgorythm::Chacha20Poly1305;

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
    #[error("error deriving encryption key: {0}")]
    DeriveKeyError(#[from] crate::hash::DeriveKeyError),
}

#[derive(Debug, thiserror::Error)]
pub enum EncryptError {
    #[error("error marshalling data: {0}")]
    MarshalError(#[from] postcard::Error),
    #[error("error deriving encryption key: {0}")]
    DeriveKeyError(#[from] crate::hash::DeriveKeyError),
    #[error("error loading key into ring: {0}")]
    CreateUnboundError(ring::error::Unspecified),
    #[error("error sealing data: {0}")]
    SealError(ring::error::Unspecified),
}

impl EncryptedData {
    pub async fn encrypt_with_password(
        data: &[u8],
        password: Vec<u8>,
    ) -> Result<Self, EncryptError> {
        let parameters = ArgonParams::strong();
        let encryption_key = parameters
            .derive_key(password)
            .await
            .map_err(EncryptError::DeriveKeyError)?;
        EncryptedData::encrypt_with_alg(data, encryption_key, DEFAULT_ALG, Some(parameters))
    }

    pub fn encrypt_with_key(data: &[u8], encryption_key: [u8; 32]) -> Result<Self, EncryptError> {
        Self::encrypt_with_alg(data, encryption_key, DEFAULT_ALG, None)
    }

    fn encrypt_with_alg(
        data: &[u8],
        encryption_key: [u8; 32],
        alg: EncryptionAlgorythm,
        parameters: Option<ArgonParams>,
    ) -> Result<Self, EncryptError> {
        let ring_alg = alg.into();

        let unbound =
            UnboundKey::new(ring_alg, &encryption_key).map_err(EncryptError::CreateUnboundError)?;
        let mut sealing = SealingKey::new(unbound, NonceCounter::new());

        let mut encrypted = data.to_owned();

        sealing
            .seal_in_place_append_tag(Aad::empty(), &mut encrypted)
            .map_err(EncryptError::SealError)?;

        Ok(Self {
            alg,
            parameters,
            ciphertext: encrypted,
        })
    }

    pub async fn decrypt_with_password(self, password: Vec<u8>) -> Result<Vec<u8>, DecryptError> {
        let parameters = if let Some(parameters) = &self.parameters {
            parameters
        } else {
            return Err(DecryptError::MissingHashParameters);
        };

        let encryption_key = parameters.derive_key(password).await?;

        self.decrypt_with_key(encryption_key)
    }

    pub fn decrypt_with_key(mut self, encryption_key: [u8; 32]) -> Result<Vec<u8>, DecryptError> {
        let ring_alg = self.alg.into();

        let unbound =
            UnboundKey::new(ring_alg, &encryption_key).map_err(DecryptError::CreateUnboundError)?;
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
        let msg2 = encrypted
            .decrypt_with_password(password.to_owned())
            .await
            .unwrap();

        assert_eq!(msg.as_ref(), &msg2);
    }
}
