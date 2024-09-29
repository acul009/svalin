use std::ops::Deref;

use anyhow::{Context, Result};
use serde::{
    de::{DeserializeOwned, Visitor},
    Deserialize, Serialize,
};

use crate::{
    signed_message::{Sign, Verify},
    verifier::Verifier,
    Certificate, PermCredentials,
};

#[derive(Debug, Clone)]
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

    // TODO: make accessing object impossible without verifying first - probably
    // need to implement verifier
    pub fn unpack<V: Verifier>(self, verifier: V) -> T {
        todo!();
        self.object
    }
}

impl<T> Deref for SignedObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

struct SignedObjectVisitor<T>(std::marker::PhantomData<T>);

impl<'de, T> Visitor<'de> for SignedObjectVisitor<T>
where
    T: DeserializeOwned,
{
    type Value = SignedObject<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a variable length blob of data")
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        SignedObject::from_bytes(v).map_err(serde::de::Error::custom)
    }

    fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_byte_buf(v.to_vec())
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

impl<T> Serialize for SignedObject<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.to_bytes())
    }
}

impl<'de, T> Deserialize<'de> for SignedObject<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_byte_buf(SignedObjectVisitor(std::marker::PhantomData))
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
