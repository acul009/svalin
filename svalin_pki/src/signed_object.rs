use std::ops::Deref;

use anyhow::Result;
use serde::Deserialize;

use crate::{signed_message::Sign, Certificate, PermCredentials};

struct SignedObject<T> {
    object: T,
    raw: Vec<u8>,
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
    T: Deserialize<'a>,
{
    pub fn from_bytes(bytes: Vec<u8>) -> Result<SignedObject<T>, impl std::error::Error> {
        todo!()
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
    pub fn to_bytes(&self) -> Vec<u8> {
        todo!()
    }
}
