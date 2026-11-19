use std::{marker::PhantomData, ops::Deref};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use structured_digest::prelude::*;

use crate::{Certificate, Credential, SpkiHash, verifier::Verifier};

#[derive(Debug, thiserror::Error)]
pub enum SignedObjectError {
    #[error("failed to verify signer certificate")]
    CertificateVerification(#[from] crate::VerifyError),
    #[error("failed to verify signature of signed object")]
    SignatureVerification(ring::error::Unspecified),
    #[error("failed to deserialize signed object")]
    Deserialize(#[from] rmp_serde::decode::Error),
}

impl From<ring::error::Unspecified> for SignedObjectError {
    fn from(error: ring::error::Unspecified) -> Self {
        Self::SignatureVerification(error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A signed Object allows you to take a serializeable type and sign it.
/// After signing it, you can only access the data again after verifying it.
pub struct SignedObject<T> {
    payload: Vec<u8>,
    signature: Vec<u8>,
    signer_spki_hash: SpkiHash,
    phantom: PhantomData<T>,
}

impl<T> SignedObject<T>
where
    T: Serialize,
{
    pub fn new(object: &T, credentials: &Credential) -> Self {
        let payload =
            rmp_serde::to_vec_named(object).expect("object should not fail serialization");

        let signature = credentials.keypair().signing_keypair().sign(&payload);
        let signature = signature.as_ref().to_vec();

        Self {
            payload,
            signature,
            signer_spki_hash: credentials.certificate().spki_hash().clone(),
            phantom: PhantomData,
        }
    }

    pub fn signed_by(&self) -> &SpkiHash {
        &self.signer_spki_hash
    }
}

impl<T> Digestable for SignedObject<T> {
    fn digest(&self, hasher: &mut impl structured_digest::Hasher) -> structured_digest::Finished {
        hasher
            .digest_struct()
            .field("payload", &self.payload.as_slice())
            .field("signature", self.signature.as_slice())
            .field("signer", self.signer_spki_hash.as_slice())
            .finish()
    }
}

impl<T> SignedObject<T>
where
    T: DeserializeOwned,
{
    pub fn verify_signature(
        self,
        signer: Certificate,
    ) -> Result<VerifiedObject<T>, SignedObjectError> {
        let public_key =
            ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, signer.public_key());
        public_key.verify(&self.payload, &self.signature)?;

        let object = rmp_serde::from_slice(&self.payload)?;

        Ok(VerifiedObject {
            signed_object: self,
            object,
            signed_by: signer.clone(),
        })
    }

    pub async fn verify(
        self,
        verifier: &impl Verifier,
        time: u64,
    ) -> Result<VerifiedObject<T>, SignedObjectError> {
        let signer = verifier
            .verify_spki_hash(&self.signer_spki_hash, time)
            .await?;

        self.verify_signature(signer)
    }
}

// This explicitly shouldn't be serializable
#[derive(Debug)]
pub struct VerifiedObject<T> {
    signed_object: SignedObject<T>,
    object: T,
    signed_by: Certificate,
}

impl<T: Serialize> VerifiedObject<T> {
    pub fn new(object: T, credentials: &Credential) -> Self {
        let signed = SignedObject::new(&object, credentials);
        Self {
            signed_object: signed,
            object,
            signed_by: credentials.certificate().clone(),
        }
    }

    pub fn signed_by(&self) -> &Certificate {
        &self.signed_by
    }

    pub fn unpack(self) -> T {
        self.object
    }

    pub fn pack(&self) -> &SignedObject<T> {
        &self.signed_object
    }

    pub fn pack_owned(self) -> SignedObject<T> {
        self.signed_object
    }
}

impl<T> AsRef<T> for VerifiedObject<T> {
    fn as_ref(&self) -> &T {
        &self.object
    }
}

impl<T> Deref for VerifiedObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

#[cfg(test)]
mod tests {

    use serde::{Deserialize, Serialize};
    use tokio::test;

    use crate::{
        Credential, get_current_timestamp,
        signed_object::{SignedObject, SignedObjectError},
        verifier::exact::ExactVerififier,
    };

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct TestSign {
        name: String,
        age: u32,
        blob: Vec<u8>,
    }

    #[test]
    async fn test_signed_object() {
        let object = TestSign {
            name: "test".to_string(),
            age: 32,
            blob: vec![1, 2, 3],
        };

        let credentials = Credential::generate_root().unwrap();

        let signed = super::SignedObject::new(&object, &credentials);

        let encoded = rmp_serde::to_vec_named(&signed).unwrap();
        let signed2: SignedObject<TestSign> = rmp_serde::from_slice(&encoded).unwrap();

        let verifier = ExactVerififier::new(credentials.certificate().clone());

        let verified = signed2
            .verify(&verifier, get_current_timestamp())
            .await
            .unwrap();

        assert_eq!(verified.unpack(), object);
    }

    #[test]
    async fn test_tampered_object() {
        let object = TestSign {
            name: "test".to_string(),
            age: 32,
            blob: vec![1, 2, 3],
        };

        let credentials = Credential::generate_root().unwrap();

        let mut signed = super::SignedObject::new(&object, &credentials);
        let verifier = ExactVerififier::new(credentials.certificate().clone());

        let object2 = TestSign {
            name: "tset".to_string(),
            age: 33,
            blob: vec![4, 5, 6],
        };

        let data = rmp_serde::to_vec_named(&object2).unwrap();

        signed.payload[0..data.len()].copy_from_slice(&data);

        let _verified_err = signed
            .verify(&verifier, get_current_timestamp())
            .await
            .unwrap_err();
    }

    #[test]
    async fn test_modified_signature() {
        let credentials = Credential::generate_root().unwrap();
        let mut signed = SignedObject::new(&42u64, &credentials);
        let signer = credentials.certificate().clone();

        assert_eq!(
            signed
                .clone()
                .verify_signature(signer.clone())
                .unwrap()
                .unpack(),
            42
        );
        signed.signature[0] ^= 1;

        assert!(matches!(
            signed.verify_signature(signer),
            Err(SignedObjectError::SignatureVerification(_))
        ));
    }

    #[test]
    async fn test_wrong_public_key() {
        let credentials = Credential::generate_root().unwrap();
        let other_credentials = Credential::generate_root().unwrap();
        let signed = SignedObject::new(&42u64, &credentials);

        assert_eq!(
            signed
                .clone()
                .verify_signature(credentials.certificate().clone())
                .unwrap()
                .unpack(),
            42
        );
        assert!(matches!(
            signed.verify_signature(other_credentials.certificate().clone()),
            Err(SignedObjectError::SignatureVerification(_))
        ));
    }

    #[test]
    async fn test_malformed_signature() {
        let credentials = Credential::generate_root().unwrap();
        let mut signed = SignedObject::new(&42u64, &credentials);
        let signer = credentials.certificate().clone();
        let valid_signature = signed.signature.clone();

        for length in [0, 1, 63, 65] {
            signed.signature = valid_signature.clone();
            signed.signature.resize(length, 0);
            assert!(
                matches!(
                    signed.clone().verify_signature(signer.clone()),
                    Err(SignedObjectError::SignatureVerification(_))
                ),
                "accepted a signature with length {length}"
            );
        }
    }

    #[test]
    async fn test_wrong_signer() {
        let object = TestSign {
            name: "test".to_string(),
            age: 32,
            blob: vec![1, 2, 3],
        };

        let credentials = Credential::generate_root().unwrap();
        let credentials2 = Credential::generate_root().unwrap();

        let signed = super::SignedObject::new(&object, &credentials);
        let verifier = ExactVerififier::new(credentials2.certificate().clone());

        let _verified_err = signed
            .verify(&verifier, get_current_timestamp())
            .await
            .unwrap_err();
    }
}
