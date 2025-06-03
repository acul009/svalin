use std::collections::HashMap;

use ring::{
    agreement::{EphemeralPrivateKey, UnparsedPublicKey, X25519},
    hkdf::{HKDF_SHA512, KeyType},
    rand::SystemRandom,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Certificate, EncryptError, EncryptedData, EncryptedObject, GenerateKeyError, generate_key,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct HybridEncryptedObject<T> {
    encrypted_object: EncryptedObject<T>,
    shared_keys: HashMap<[u8; 32], HybridEncryptionKeyData>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HybridEncryptionKeyData {
    ephemeral_public_key: Vec<u8>,
    encrypted_symmetric_key: EncryptedData,
}

#[derive(Error, Debug)]
pub enum HybridEncryptedObjectError {
    #[error("Failed to generate encryption key: {0}")]
    GenerateKey(#[from] GenerateKeyError),
    #[error("Unspecified error")]
    Unspecified(ring::error::Unspecified),
    #[error("Symmetric encryption error: {0}")]
    SymmetricError(#[from] EncryptError),
}

impl From<ring::error::Unspecified> for HybridEncryptedObjectError {
    fn from(error: ring::error::Unspecified) -> Self {
        Self::Unspecified(error)
    }
}

impl<T> HybridEncryptedObject<T>
where
    T: Serialize,
{
    pub fn encrypt_with_receivers(
        object: &T,
        receivers: Vec<Certificate>,
    ) -> Result<Self, HybridEncryptedObjectError> {
        let symmetric_key = generate_key()?;
        let encrypted_object = EncryptedObject::encrypt_with_key(object, symmetric_key.clone())?;

        let mut shared_keys = HashMap::new();
        for receiver in receivers {
            let receiver_public_key = receiver.public_key();
            let ephemeral_private_key =
                EphemeralPrivateKey::generate(&X25519, &SystemRandom::new())?;
            let ephemeral_public_key = ephemeral_private_key.compute_public_key()?;

            let shared_key = ring::agreement::agree_ephemeral(
                ephemeral_private_key,
                &UnparsedPublicKey::new(&X25519, receiver.public_key()),
                |shared_secret| {
                    let context = [ephemeral_public_key.as_ref(), receiver_public_key];
                    hdkf_kdf(shared_secret, &context)
                },
            )??;

            let encrypted_symmetric_key =
                EncryptedData::encrypt_with_key(&symmetric_key, shared_key)?;

            let encryption_data = HybridEncryptionKeyData {
                ephemeral_public_key: ephemeral_public_key.as_ref().to_vec(),
                encrypted_symmetric_key,
            };

            shared_keys.insert(receiver.fingerprint(), encryption_data);
        }

        Ok(Self {
            encrypted_object,
            shared_keys,
        })
    }
}

struct SymmetricKeyLength;

impl KeyType for SymmetricKeyLength {
    fn len(&self) -> usize {
        32
    }
}

/// Fixed salt for HKDF
/// generated using: `openssl rand -hex 64 | sed 's/\(..\)/0x\1, /g' | sed 's/, $//'`
const FIXED_SALT: [u8; 64] = [
    0xd3, 0x78, 0x4c, 0xaa, 0x10, 0x33, 0xcc, 0xfe, 0x28, 0xea, 0x27, 0xe6, 0x49, 0xcf, 0x41, 0x51,
    0x4b, 0x47, 0xa3, 0x9d, 0xbf, 0x7c, 0xd2, 0x15, 0xa3, 0xa7, 0xd3, 0xe4, 0xc9, 0x2d, 0x32, 0x09,
    0x9b, 0x1a, 0x17, 0xbd, 0x47, 0x13, 0x69, 0x5a, 0x00, 0xdb, 0x1f, 0xc5, 0x49, 0x99, 0x81, 0x1a,
    0x8d, 0x66, 0x00, 0xe9, 0x00, 0x88, 0x6a, 0x72, 0x4f, 0xee, 0xdf, 0x1d, 0xf0, 0x47, 0x20, 0x00,
];

fn hdkf_kdf(
    shared_secret: &[u8],
    context: &[&[u8]],
) -> Result<[u8; 32], HybridEncryptedObjectError> {
    let salt = ring::hkdf::Salt::new(HKDF_SHA512, &FIXED_SALT);
    let prk = salt.extract(shared_secret);
    let okm = prk.expand(context, SymmetricKeyLength);

    match okm {
        Ok(okm) => {
            let mut out = [0u8; 32];
            match okm.fill(&mut out) {
                Ok(()) => Ok(out),
                Err(err) => Err(err.into()),
            }
        }
        Err(err) => Err(err.into()),
    }
}
