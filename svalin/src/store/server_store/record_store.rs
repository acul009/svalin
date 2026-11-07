use serde::{Deserialize, Serialize};
use std::sync::Arc;

use svalin_pki::SpkiHash;

#[derive(Debug)]
pub struct RecordStore {
    pool: sqlx::SqlitePool,
}

impl RecordStore {
    pub(crate) fn open(pool: sqlx::SqlitePool) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

/// Identifies a persistent record whose encrypted contents can be replaced.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordId {
    DeviceReport(SpkiHash),
    MetaInfo(SpkiHash),
}

impl RecordId {
    pub fn to_bytes(&self) -> Vec<u8> {
        let (prefix, spki_hash) = match self {
            Self::DeviceReport(spki_hash) => (b"report/".as_slice(), spki_hash),
            Self::MetaInfo(spki_hash) => (b"meta/".as_slice(), spki_hash),
        };
        let mut bytes = prefix.to_vec();
        bytes.extend_from_slice(spki_hash.as_slice());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ParseRecordIdError> {
        let separator = bytes.iter().position(|byte| *byte == b'/');
        let (kind, hash) = match separator {
            Some(index) => (&bytes[..index], Some(&bytes[index + 1..])),
            None => (bytes, None),
        };
        let constructor: fn(SpkiHash) -> Self = match kind {
            b"report" => Self::DeviceReport,
            b"meta" => Self::MetaInfo,
            _ => return Err(ParseRecordIdError::UnknownRecordType),
        };
        let hash = hash.ok_or(ParseRecordIdError::MissingData)?;
        let hash = SpkiHash::from_slice(hash).map_err(|_| ParseRecordIdError::InvalidSpkiHash)?;
        Ok(constructor(hash))
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseRecordIdError {
    #[error("unknown record type")]
    UnknownRecordType,
    #[error("missing record data")]
    MissingData,
    #[error("invalid SPKI hash")]
    InvalidSpkiHash,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_record_ids() {
        let hash = SpkiHash::from_slice(&[42; 32]).unwrap();
        for id in [
            RecordId::DeviceReport(hash.clone()),
            RecordId::MetaInfo(hash),
        ] {
            assert_eq!(RecordId::from_bytes(&id.to_bytes()).unwrap(), id);
            let encoded = rmp_serde::to_vec_named(&id).unwrap();
            assert_eq!(rmp_serde::from_slice::<RecordId>(&encoded).unwrap(), id);
        }
    }

    #[test]
    fn rejects_malformed_ids() {
        for bytes in [
            b"".as_slice(),
            b"unknown/hash",
            b"report",
            b"meta/",
            b"report/not-a-hash",
            b"meta/0000",
        ] {
            assert!(RecordId::from_bytes(bytes).is_err());
        }
        let id = RecordId::DeviceReport(SpkiHash::from_slice(&[42; 32]).unwrap());
        let mut bytes = id.to_bytes();
        bytes.extend_from_slice(b"/extra");
        assert_eq!(
            RecordId::from_bytes(&bytes),
            Err(ParseRecordIdError::InvalidSpkiHash)
        );
    }
}
