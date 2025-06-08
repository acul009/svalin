use std::collections::HashMap;

use ed25519_dalek::{SigningKey, VerifyingKey, pkcs8::DecodePrivateKey};
use hpke::{HpkeError, Serializable};
use rand::{SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use zeroize::Zeroize;

use crate::{
    Certificate, DecryptError, EncryptError, EncryptedObject, GenerateKeyError, PermCredentials,
    generate_key,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HybridEncryptedObject<T> {
    encrypted_object: EncryptedObject<T>,
    shared_keys: HashMap<String, HybridEncryptionKeyData>,
}

/// ciphersuite for HPKE used for sharing the main symmetric key
type Kem = hpke::kem::X25519HkdfSha256;
type Aead = hpke::aead::ChaCha20Poly1305;
type Kdf = hpke::kdf::HkdfSha512;

#[derive(Serialize, Deserialize, Debug, Clone)]
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
    #[error("Key conversion error: {0}")]
    KeyConversionError(String),
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

            // Convert Ed25519 public key to X25519 public key
            let x25519_public_key = ed25519_to_x25519_public(receiver.public_key())
                .map_err(|e| HybridEncryptedObjectError::KeyConversionError(e))?;

            let receiver_public_key =
                <<Kem as hpke::kem::Kem>::PublicKey as hpke::Deserializable>::from_bytes(
                    &x25519_public_key,
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

            shared_keys.insert(fingerprint_to_hex(&receiver.fingerprint()), encryption_data);
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
        let shared_key_data = if let Some(shared_key_data) = self.shared_keys.remove(
            &fingerprint_to_hex(&credentials.get_certificate().fingerprint()),
        ) {
            shared_key_data
        } else {
            return Err(HybridEncryptedObjectError::NotIntendedForMe);
        };

        // Convert Ed25519 private key to X25519 private key
        let x25519_private_key = ed25519_to_x25519_private(credentials.get_der_key_bytes())
            .map_err(|e| HybridEncryptedObjectError::KeyConversionError(e))?;

        let private_key =
            <<Kem as hpke::kem::Kem>::PrivateKey as hpke::Deserializable>::from_bytes(
                &x25519_private_key,
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

/// Convert Ed25519 public key (DER format) to X25519 public key (32 bytes)
fn ed25519_to_x25519_public(ed25519_public_der: &[u8]) -> Result<[u8; 32], String> {
    // Extract the raw 32-byte public key from DER format
    // Ed25519 public keys in DER format have the raw key at the end
    if ed25519_public_der.len() < 32 {
        return Err("Ed25519 public key too short".to_string());
    }

    let raw_ed25519_key = &ed25519_public_der[ed25519_public_der.len() - 32..];
    let mut ed25519_bytes = [0u8; 32];
    ed25519_bytes.copy_from_slice(raw_ed25519_key);

    let ed25519_key = VerifyingKey::from_bytes(&ed25519_bytes)
        .map_err(|e| format!("Invalid Ed25519 public key: {}", e))?;

    // Convert to X25519 - this is a well-known conversion
    let x25519_key = ed25519_key.to_montgomery();
    Ok(x25519_key.to_bytes())
}

/// Convert Ed25519 private key (DER format) to X25519 private key (32 bytes)
fn ed25519_to_x25519_private(ed25519_private_der: &[u8]) -> Result<[u8; 32], String> {
    // Parse the DER-encoded private key
    let signing_key = SigningKey::from_pkcs8_der(ed25519_private_der)
        .map_err(|e| format!("Failed to parse Ed25519 private key: {}", e))?;

    // Convert to X25519 - this is a well-known conversion
    let x25519_key = signing_key.to_scalar_bytes();
    Ok(x25519_key)
}

/// Convert a certificate fingerprint to a hex string
fn fingerprint_to_hex(fingerprint: &[u8; 32]) -> String {
    fingerprint.iter().map(|b| format!("{:02x}", b)).collect()
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
    use crate::Keypair;
    use ring::rand::{SecureRandom, SystemRandom};

    fn generate_test_data(size: usize) -> Vec<u8> {
        let rand = SystemRandom::new();
        let mut data = vec![0u8; size];
        rand.fill(&mut data).unwrap();
        data
    }

    fn create_test_credentials() -> PermCredentials {
        Keypair::generate().to_self_signed_cert().unwrap()
    }

    #[test]
    fn test_single_receiver_encrypt_decrypt() {
        let test_data = generate_test_data(1024);
        let receiver_creds = create_test_credentials();
        let receiver_cert = receiver_creds.get_certificate().clone();

        // Encrypt for single receiver
        let encrypted =
            HybridEncryptedObject::encrypt_with_receivers(&test_data, vec![receiver_cert]).unwrap();

        // Decrypt with correct credentials
        let decrypted = encrypted.decrypt(receiver_creds).unwrap();

        assert_eq!(test_data, decrypted);
    }

    #[test]
    fn test_multiple_receivers_encrypt_decrypt() {
        let test_data = generate_test_data(512);

        let receiver1_creds = create_test_credentials();
        let receiver2_creds = create_test_credentials();
        let receiver3_creds = create_test_credentials();

        let receiver1_cert = receiver1_creds.get_certificate().clone();
        let receiver2_cert = receiver2_creds.get_certificate().clone();
        let receiver3_cert = receiver3_creds.get_certificate().clone();

        // Encrypt for multiple receivers
        let encrypted = HybridEncryptedObject::encrypt_with_receivers(
            &test_data,
            vec![receiver1_cert, receiver2_cert, receiver3_cert],
        )
        .unwrap();

        // Each receiver should be able to decrypt
        let decrypted1 = encrypted.clone().decrypt(receiver1_creds).unwrap();
        let decrypted2 = encrypted.clone().decrypt(receiver2_creds).unwrap();
        let decrypted3 = encrypted.decrypt(receiver3_creds).unwrap();

        assert_eq!(test_data, decrypted1);
        assert_eq!(test_data, decrypted2);
        assert_eq!(test_data, decrypted3);
    }

    #[test]
    fn test_empty_data_encrypt_decrypt() {
        let test_data: Vec<u8> = vec![];
        let receiver_creds = create_test_credentials();
        let receiver_cert = receiver_creds.get_certificate().clone();

        let encrypted =
            HybridEncryptedObject::encrypt_with_receivers(&test_data, vec![receiver_cert]).unwrap();

        let decrypted = encrypted.decrypt(receiver_creds).unwrap();
        assert_eq!(test_data, decrypted);
    }

    #[test]
    fn test_large_data_encrypt_decrypt() {
        let test_data = generate_test_data(1024 * 1024); // 1MB
        let receiver_creds = create_test_credentials();
        let receiver_cert = receiver_creds.get_certificate().clone();

        let encrypted =
            HybridEncryptedObject::encrypt_with_receivers(&test_data, vec![receiver_cert]).unwrap();

        let decrypted = encrypted.decrypt(receiver_creds).unwrap();
        assert_eq!(test_data, decrypted);
    }

    #[test]
    fn test_decrypt_with_wrong_credentials() {
        let test_data = generate_test_data(256);
        let receiver_creds = create_test_credentials();
        let wrong_creds = create_test_credentials();
        let receiver_cert = receiver_creds.get_certificate().clone();

        let encrypted =
            HybridEncryptedObject::encrypt_with_receivers(&test_data, vec![receiver_cert]).unwrap();

        // Try to decrypt with wrong credentials
        let result = encrypted.decrypt(wrong_creds);
        assert!(matches!(
            result,
            Err(HybridEncryptedObjectError::NotIntendedForMe)
        ));
    }

    #[test]
    fn test_encrypt_with_no_receivers() {
        let test_data = generate_test_data(128);

        let encrypted = HybridEncryptedObject::encrypt_with_receivers(&test_data, vec![]).unwrap();

        // Should succeed but nobody can decrypt it
        assert!(encrypted.shared_keys.is_empty());
    }

    #[test]
    fn test_hybrid_encrypted_object_serialization() {
        let test_data = generate_test_data(256);
        let receiver_creds = create_test_credentials();
        let receiver_cert = receiver_creds.get_certificate().clone();

        let encrypted =
            HybridEncryptedObject::encrypt_with_receivers(&test_data, vec![receiver_cert]).unwrap();

        // Test postcard serialization
        let serialized = postcard::to_stdvec(&encrypted).unwrap();
        let deserialized: HybridEncryptedObject<Vec<u8>> =
            postcard::from_bytes(&serialized).unwrap();

        // Should be able to decrypt the deserialized version
        let decrypted = deserialized.decrypt(receiver_creds).unwrap();
        assert_eq!(test_data, decrypted);
    }

    #[test]
    fn test_hybrid_encrypted_object_json_serialization() {
        let test_data = generate_test_data(128);
        let receiver_creds = create_test_credentials();
        let receiver_cert = receiver_creds.get_certificate().clone();

        let encrypted =
            HybridEncryptedObject::encrypt_with_receivers(&test_data, vec![receiver_cert]).unwrap();

        // Test JSON serialization
        let json = serde_json::to_string(&encrypted).unwrap();
        let deserialized: HybridEncryptedObject<Vec<u8>> = serde_json::from_str(&json).unwrap();

        // Should be able to decrypt the deserialized version
        let decrypted = deserialized.decrypt(receiver_creds).unwrap();
        assert_eq!(test_data, decrypted);
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestStruct {
        name: String,
        value: u64,
        data: Vec<u8>,
    }

    #[test]
    fn test_custom_struct_encrypt_decrypt() {
        let test_struct = TestStruct {
            name: "test_object".to_string(),
            value: 42,
            data: generate_test_data(64),
        };

        let receiver_creds = create_test_credentials();
        let receiver_cert = receiver_creds.get_certificate().clone();

        let encrypted =
            HybridEncryptedObject::encrypt_with_receivers(&test_struct, vec![receiver_cert])
                .unwrap();

        let decrypted = encrypted.decrypt(receiver_creds).unwrap();
        assert_eq!(test_struct, decrypted);
    }

    #[test]
    fn test_duplicate_receivers() {
        let test_data = generate_test_data(256);
        let receiver_creds = create_test_credentials();
        let receiver_cert = receiver_creds.get_certificate().clone();

        // Add the same receiver twice
        let encrypted = HybridEncryptedObject::encrypt_with_receivers(
            &test_data,
            vec![receiver_cert.clone(), receiver_cert],
        )
        .unwrap();

        // Should only have one entry in shared_keys (HashMap deduplication)
        assert_eq!(encrypted.shared_keys.len(), 1);

        // Should still be able to decrypt
        let decrypted = encrypted.decrypt(receiver_creds).unwrap();
        assert_eq!(test_data, decrypted);
    }

    #[test]
    fn test_mixed_receivers_partial_access() {
        let test_data = generate_test_data(256);

        let receiver1_creds = create_test_credentials();
        let receiver2_creds = create_test_credentials();
        let non_receiver_creds = create_test_credentials();

        let receiver1_cert = receiver1_creds.get_certificate().clone();
        let receiver2_cert = receiver2_creds.get_certificate().clone();

        // Encrypt for only two receivers
        let encrypted = HybridEncryptedObject::encrypt_with_receivers(
            &test_data,
            vec![receiver1_cert, receiver2_cert],
        )
        .unwrap();

        // Authorized receivers should succeed
        let decrypted1 = encrypted.clone().decrypt(receiver1_creds).unwrap();
        let decrypted2 = encrypted.clone().decrypt(receiver2_creds).unwrap();

        // Non-receiver should fail
        let result = encrypted.decrypt(non_receiver_creds);

        assert_eq!(test_data, decrypted1);
        assert_eq!(test_data, decrypted2);
        assert!(matches!(
            result,
            Err(HybridEncryptedObjectError::NotIntendedForMe)
        ));
    }
}
