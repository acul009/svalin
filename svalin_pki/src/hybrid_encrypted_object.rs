use std::collections::HashMap;

use hpke::{HpkeError, Serializable};
use rand::{SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use zeroize::Zeroize;

use crate::{
    Certificate, DecryptError, EncryptError, EncryptedObject, GenerateKeyError, PermCredentials,
    generate_key,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct HybridEncryptedObject<T> {
    encrypted_object: EncryptedObject<T>,
    shared_keys: HashMap<[u8; 32], HybridEncryptionKeyData>,
}

/// ciphersuite for HPKE used for sharing the main symmetric key
type Kem = hpke::kem::X25519HkdfSha256;
type Aead = hpke::aead::ChaCha20Poly1305;
type Kdf = hpke::kdf::HkdfSha512;

#[derive(Serialize, Deserialize, Debug)]
pub struct HybridEncryptionKeyData {
    shared_info_value: [u8; 32],
    encrypted_symmetric_key: Vec<u8>,
    encapsulated_key: Vec<u8>,
}

#[derive(Error, Debug)]
pub enum HybridEncryptedObjectError {
    #[error("Failed to generate encryption key: {0}")]
    GenerateKey(#[from] GenerateKeyError),
    #[error("Unspecified error")]
    Unspecified(ring::error::Unspecified),
    #[error("Symmetric encryption error: {0}")]
    EncryptError(#[from] EncryptError),
    #[error("not encrypted for the given credentials")]
    NotIntendedForMe,
    #[error("HPKE error: {0}")]
    HpkeError(HpkeError),
    #[error("decoded symmetric key has the wrong length")]
    InvalidKeyLength,
    #[error("Symmetric decryption error: {0}")]
    DecryptError(#[from] DecryptError),
}

impl From<HpkeError> for HybridEncryptedObjectError {
    fn from(error: HpkeError) -> Self {
        Self::HpkeError(error)
    }
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

        // random generator for hpke
        let mut csprng = StdRng::from_os_rng();

        let mut shared_keys = HashMap::new();
        for receiver in receivers {
            let shared_info_value = generate_key()?;

            let receiver_public_key =
                <<Kem as hpke::kem::Kem>::PublicKey as hpke::Deserializable>::from_bytes(
                    receiver.public_key(),
                )?;

            let (encapsulated_key, encrypted_symmetric_key) =
                hpke::single_shot_seal::<Aead, Kdf, Kem, _>(
                    &hpke::OpModeS::Base,
                    &receiver_public_key,
                    &shared_info_value,
                    &symmetric_key,
                    &[],
                    &mut csprng,
                )?;

            let encapsulated_key = encapsulated_key.to_bytes().to_vec();

            let encryption_data = HybridEncryptionKeyData {
                shared_info_value,
                encrypted_symmetric_key,
                encapsulated_key,
            };

            shared_keys.insert(receiver.fingerprint(), encryption_data);
        }

        {
            let mut symmetric_key = symmetric_key;
            symmetric_key.zeroize();
        }

        Ok(Self {
            encrypted_object,
            shared_keys,
        })
    }
}

impl<T> HybridEncryptedObject<T>
where
    T: DeserializeOwned,
{
    pub fn decrypt(
        mut self,
        credentials: PermCredentials,
    ) -> Result<T, HybridEncryptedObjectError> {
        let shared_key_data = if let Some(shared_key_data) = self
            .shared_keys
            .remove(&credentials.get_certificate().fingerprint())
        {
            shared_key_data
        } else {
            return Err(HybridEncryptedObjectError::NotIntendedForMe);
        };

        let private_key =
            <<Kem as hpke::kem::Kem>::PrivateKey as hpke::Deserializable>::from_bytes(
                credentials.get_der_key_bytes(),
            )?;

        let encapsulated_key =
            <<Kem as hpke::kem::Kem>::EncappedKey as hpke::Deserializable>::from_bytes(
                &shared_key_data.encapsulated_key,
            )?;

        let symmetric_key = hpke::single_shot_open::<Aead, Kdf, Kem>(
            &hpke::OpModeR::Base,
            &private_key,
            &encapsulated_key,
            &shared_key_data.shared_info_value,
            &shared_key_data.encrypted_symmetric_key,
            &[],
        )?;

        let symmetric_key = symmetric_key
            .try_into()
            .map_err(|_| HybridEncryptedObjectError::InvalidKeyLength)?;

        let object = self.encrypted_object.decrypt_with_key(symmetric_key)?;

        Ok(object)
    }
}

// /// Fixed salt for HKDF
// ///
// /// generated using: `openssl rand -hex 64 | sed 's/\(..\)/0x\1, /g' | sed 's/, $//'`
// const FIXED_SALT: [u8; 64] = [
//     0xd3, 0x78, 0x4c, 0xaa, 0x10, 0x33, 0xcc, 0xfe, 0x28, 0xea, 0x27, 0xe6, 0x49, 0xcf, 0x41, 0x51,
//     0x4b, 0x47, 0xa3, 0x9d, 0xbf, 0x7c, 0xd2, 0x15, 0xa3, 0xa7, 0xd3, 0xe4, 0xc9, 0x2d, 0x32, 0x09,
//     0x9b, 0x1a, 0x17, 0xbd, 0x47, 0x13, 0x69, 0x5a, 0x00, 0xdb, 0x1f, 0xc5, 0x49, 0x99, 0x81, 0x1a,
//     0x8d, 0x66, 0x00, 0xe9, 0x00, 0x88, 0x6a, 0x72, 0x4f, 0xee, 0xdf, 0x1d, 0xf0, 0x47, 0x20, 0x00,
// ];

// fn hdkf_kdf(
//     shared_secret: &[u8],
//     sender_public_key: &[u8],
//     receiver_public_key: &[u8],
//     unique_context: &[u8],
// ) -> Result<[u8; 32], HybridEncryptedObjectError> {
//     let context = [sender_public_key, receiver_public_key, unique_context];

//     let salt = ring::hkdf::Salt::new(HKDF_SHA512, &FIXED_SALT);
//     let prk = salt.extract(shared_secret);
//     let okm = prk.expand(&context, SymmetricKeyLength);

//     match okm {
//         Ok(okm) => {
//             let mut out = [0u8; 32];
//             match okm.fill(&mut out) {
//                 Ok(()) => Ok(out),
//                 Err(err) => Err(err.into()),
//             }
//         }
//         Err(err) => Err(err.into()),
//     }
// }

#[cfg(test)]
mod test {
    use super::*;
    use ring::rand::{SecureRandom, SystemR
