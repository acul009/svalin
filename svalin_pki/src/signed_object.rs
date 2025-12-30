use std::{marker::PhantomData, ops::Deref};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    Certificate, Credential, SpkiHash,
    signed_message::{Sign, Verify},
    verifier::Verifier,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A signed Object allows you to take a serializeable type and sign it.
/// After signing it, you can only access the data again using a verifier.
pub struct SignedObject<T> {
    raw: Vec<u8>,
    singer_spki_hash: SpkiHash,
    phantom: PhantomData<T>,
}

impl<T> SignedObject<T>
where
    T: Serialize,
{
    pub fn new(object: &T, credentials: &Credential) -> Result<Self> {
        let data = postcard::to_extend(object, Vec::new())?;

        let raw = credentials.sign(&data)?;

        Ok(Self {
            raw,
            singer_spki_hash: credentials.get_certificate().spki_hash().clone(),
            phantom: PhantomData,
        })
    }
}

impl<T> SignedObject<T>
where
    T: DeserializeOwned,
{
    pub async fn verify(self, verifier: &impl Verifier, time: u64) -> Result<VerifiedObject<T>> {
        let signer = verifier
            .verify_spki_hash(&self.singer_spki_hash, time)
            .await
            .context("failed to verify fingerprint of signed object")?;

        let data = signer
            .verify(&self.raw)
            .context("failed to verify signature of signed object")?;

        let object = postcard::from_bytes(&data).context("failed to deserialize signed object")?;

        Ok(VerifiedObject {
            signed_object: self,
            object,
            signed_by: signer,
        })
    }
}

// This explicitly shouldn't be serializable
#[derive(Debug)]
pub struct VerifiedObject<T> {
    signed_object: SignedObject<T>,
    object: T,
    signed_by: Certificate,
}

impl<T> VerifiedObject<T> {
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
        Credential, get_current_timestamp, signed_object::SignedObject,
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

        let signed = super::SignedObject::new(&object, &credentials).unwrap();

        let encoded = postcard::to_extend(&signed, Vec::new()).unwrap();
        let signed2: SignedObject<TestSign> = postcard::from_bytes(&encoded).unwrap();

        let verifier = ExactVerififier::new(credentials.get_certificate().clone());

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

        let mut signed = super::SignedObject::new(&object, &credentials).unwrap();
        let verifier = ExactVerififier::new(credentials.get_certificate().clone());

        let object2 = TestSign {
            name: "tset".to_string(),
            age: 33,
            blob: vec![4, 5, 6],
        };

        let data = postcard::to_extend(&object2, Vec::new()).unwrap();

        signed.raw[0..data.len()].copy_from_slice(&data);

        let _verified_err = signed
            .verify(&verifier, get_current_timestamp())
            .await
            .unwrap_err();
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

        let signed = super::SignedObject::new(&object, &credentials).unwrap();
        let verifier = ExactVerififier::new(credentials2.get_certificate().clone());

        let _verified_err = signed
            .verify(&verifier, get_current_timestamp())
            .await
            .unwrap_err();
    }
}
