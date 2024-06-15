use std::ops::Deref;

use serde::Deserialize;

use crate::{signed_message::Sign, PermCredentials};

struct SignedObject<T> {
    object: T,
    raw: Vec<u8>,
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
    pub fn new(object: &T, credentials: impl Sign) -> Self {
        todo!()
    }
}
