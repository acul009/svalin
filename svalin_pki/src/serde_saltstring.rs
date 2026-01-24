use core::fmt;
use password_hash::SaltString;
use serde::de::{Error, Visitor};
use serde::{Deserializer, Serializer};

pub fn serialize<S>(data: &SaltString, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(data.as_str())
}

struct SaltStringVisitor {}

impl<'de> Visitor<'de> for SaltStringVisitor {
    type Value = SaltString;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a valid ASCII salt string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        SaltString::from_b64(v).map_err(Error::custom)
    }
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<SaltString, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(SaltStringVisitor {})
}
