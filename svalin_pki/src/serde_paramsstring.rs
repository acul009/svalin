use core::fmt;
use password_hash::ParamsString;
use serde::de::{Error, Visitor};
use serde::{Deserializer, Serializer};

pub fn serialize<S>(data: &ParamsString, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(data.as_str())
}

pub struct ParamsStringVisitor {}

impl<'de> Visitor<'de> for ParamsStringVisitor {
    type Value = ParamsString;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a valid PHC parameter string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        v.parse().map_err(Error::custom)
    }
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<ParamsString, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(ParamsStringVisitor {})
}
