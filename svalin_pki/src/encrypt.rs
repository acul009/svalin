use anyhow::{anyhow, Context, Result};

use ring::aead::{
    Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey, AES_256_GCM,
    CHACHA20_POLY1305, NONCE_LEN,
};
use serde::{Deserialize, Serialize};

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
    parameters: ArgonParams,
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
        EncryptedData::encrypt_with_alg(data, password, DEFAULT_ALG).await
    }

    async fn encrypt_with_alg(
        data: &[u8],
        password: Vec<u8>,
        alg: EncryptionAlgorythm,
    ) -> Result<Vec<u8>> {
        let parameters = ArgonParams::basic();
        let encryption_key = parameters.derive_key(password).await?;

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
        let mut encrypted_data: EncryptedData = postcard::from_bytes(cipherdata)?;

        let encryption_key = encrypted_data.parameters.derive_key(password).await?;

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
