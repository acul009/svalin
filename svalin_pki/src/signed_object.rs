use std::ops::Deref;

use anyhow::{Context, Result};
use ring::hmac::sign;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use x509_parser::objects;

use crate::{
    signed_message::{Sign, Verify},
    Certificate, PermCredentials,
};

pub struct SignedObject<T> {
    object: T,
    raw: Vec<u8>,
    signed_by: Certificate,
}

#[derive(Serialize, Deserialize, Debug)]
struct SignedBlob {
    blob: Vec<u8>,
    signed_by: Certificate,
}

impl<T> SignedObject<T> {
    pub fn signed_by(&self) -> &Certificate {
        &self.signed_by
    }
}

impl<T> Deref for SignedObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

impl<T> SignedObject<T>
where
    T: DeserializeOwned,
{
    pub fn from_bytes(bytes: Vec<u8>) -> Result<SignedObject<T>> {
        let signed_blob: SignedBlob =
            postcard::from_bytes(&bytes).context("failed to deserialize signer certificate")?;

        let serialized_data = signed_blob
            .signed_by
            .verify(&signed_blob.blob)
            .context("failed to verify signed blob")?;

        let object: T = postcard::from_bytes(&serialized_data)
            .context("Failed to deserialize contained object")?;

        Ok(SignedObject {
            object,
            raw: bytes,
            signed_by: signed_blob.signed_by,
        })
    }
}

impl<T> SignedObject<T>
where
    T: serde::Serialize,
{
    pub fn new(object: T, credentials: &PermCredentials) -> Result<Self> {
        let encoded = postcard::to_extend(&object, Vec::new())?;

        let signed = credentials.sign(&encoded)?;

        let blob = SignedBlob {
            blob: signed,
            signed_by: credentials.get_certificate().clone(),
        };

        let raw = postcard::to_extend(&blob, Vec::new())?;

        Ok(Self {
            object,
            raw,
            signed_by: blob.signed_by,
        })
    }
}

impl<T> SignedObject<T> {
    pub fn to_bytes(&self) -> &[u8] {
        &self.raw
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;

    use serde::{Deserialize, Serialize};

    use crate::{
        signed_message::{Sign, Verify},
        Keypair,
    };

    use super::SignedBlob;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct TestSign {
        name: String,
        age: u32,
        blob: Vec<u8>,
    }

    #[test]
    fn test_signed_blob() {
        let credentials = Keypair::generate().unwrap().to_self_signed_cert().unwrap();

        let test_vec = vec![1, 2, 3];

        let signed = credentials.sign(&test_vec).unwrap();

        let blob = SignedBlob {
            blob: signed,
            signed_by: credentials.get_certificate().clone(),
        };

        let encoded = postcard::to_extend(&blob, Vec::new()).unwrap();

        let blob2: SignedBlob = postcard::from_bytes(&encoded).unwrap();

        blob2.signed_by.verify(&blob2.blob).unwrap();

        assert_eq!(blob.signed_by, blob2.signed_by);

        assert_eq!(blob.blob, blob2.blob);
    }

    #[test]
    fn test_signed_object() {
        let object = TestSign {
            name: "test".to_string(),
            age: 32,
            blob: vec![1, 2, 3],
        };

        let credentials = Keypair::generate().unwrap().to_self_signed_cert().unwrap();

        let signed = super::SignedObject::new(object, &credentials).unwrap();

        let encoded = signed.to_bytes().to_owned();
        let signed2 = super::SignedObject::<TestSign>::from_bytes(encoded).unwrap();

        assert_eq!(signed.signed_by(), signed2.signed_by());

        assert_eq!(signed.deref(), signed2.deref());
    }
}
