use std::ops::Deref;

use anyhow::Result;
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
    pub fn signed_by() {
        todo!()
    }
}

impl<T> Deref for SignedObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

impl<'a, T> SignedObject<T>
where
    T: DeserializeOwned,
{
    pub fn from_bytes(bytes: Vec<u8>) -> Result<SignedObject<T>> {
        let signed_blob: SignedBlob = postcard::from_bytes(&bytes)?;

        let serialized_data = signed_blob.signed_by.verify(&signed_blob.blob)?;

        let object: T = postcard::from_bytes(&serialized_data)?;

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
    pub fn new(object: T, credentials: PermCredentials) -> Result<Self> {
        let encoded = postcard::to_extend(&object, Vec::new())?;

        let raw = credentials.sign(&encoded)?;

        Ok(Self {
            object,
            raw,
            signed_by: credentials.get_certificate().clone(),
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

    #[test]
    fn test_signed_object() {
        todo!()
    }
}
