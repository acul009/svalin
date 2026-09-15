//! Current guest and volume selection returned by the PVE included_volumes API.

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Coverage {
    /// All guests selected by the job, potentially spanning multiple nodes.
    #[serde(alias = "children")]
    pub guests: Vec<Guest>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Guest {
    pub id: u32,
    /// Removed guests may have no name or volume information.
    pub name: Option<String>,
    /// PVE reports qemu, lxc, or unknown for a removed guest.
    #[serde(alias = "type")]
    pub guest_type: String,
    #[serde(default, alias = "children")]
    pub volumes: Vec<Volume>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Volume {
    /// Guest ID and configuration key, for example 100:scsi0.
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "included")]
    pub included: bool,
    pub reason: String,
}

// PVE returns numeric booleans, while our reports serialize native booleans.
fn included<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Bool(bool),
        Number(u8),
    }
    match Value::deserialize(deserializer)? {
        Value::Bool(value) => Ok(value),
        Value::Number(0) => Ok(false),
        Value::Number(1) => Ok(true),
        Value::Number(_) => Err(serde::de::Error::custom("expected boolean or 0/1")),
    }
}
