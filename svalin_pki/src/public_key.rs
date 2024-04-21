use anyhow::Result;
use serde::{de, Deserialize, Serialize};

pub struct PublicKey {
    raw: Vec<u8>,
}

impl PublicKey {
    pub(crate) fn from_bytes(bytes: Vec<u8>) -> Result<PublicKey> {
        Ok(PublicKey { raw: bytes })
    }

    pub(crate) fn to_bytes(&self) -> &[u8] {
        &self.raw
    }
}

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.to_bytes())
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        PublicKey::from_bytes(bytes).map_err(de::Error::custom)
    }
}
