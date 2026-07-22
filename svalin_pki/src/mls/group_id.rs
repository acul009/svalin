use openmls::group::GroupId;

use crate::SpkiHash;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SvalinGroupId {
    GlobalGroup,
    DeviceGroup(SpkiHash),
    DeviceMetaGroup(SpkiHash),
}

impl SvalinGroupId {
    pub(crate) fn to_group_id(&self) -> GroupId {
        let bytes = match self {
            SvalinGroupId::DeviceGroup(spki_hash) => {
                let mut bytes = b"device/".to_vec();
                let hex = spki_hash.to_hex();
                bytes.extend_from_slice(hex.as_bytes());
                bytes
            }
            SvalinGroupId::DeviceMetaGroup(spki_hash) => {
                let mut bytes = b"meta/".to_vec();
                let hex = spki_hash.to_hex();
                bytes.extend_from_slice(hex.as_bytes());
                bytes
            }
            SvalinGroupId::GlobalGroup => b"global".to_vec(),
        };

        GroupId::from_slice(&bytes)
    }

    pub(crate) fn from_group_id(group_id: &GroupId) -> Result<Self, ParseGroupIdError> {
        let mut parts = group_id.as_slice().split(|c| *c == b'/');
        let Some(first) = parts.next() else {
            return Err(ParseGroupIdError::MissingGroupType);
        };

        match first {
            b"device" => {
                let Some(spki_hash) = parts.next() else {
                    return Err(ParseGroupIdError::MissingData);
                };
                let spki_hash = SpkiHash::from_hex(&spki_hash)
                    .map_err(|_| ParseGroupIdError::WrongSliceLength)?;
                Ok(Self::DeviceGroup(spki_hash))
            }
            b"meta" => {
                let Some(spki_hash) = parts.next() else {
                    return Err(ParseGroupIdError::MissingData);
                };
                let spki_hash = SpkiHash::from_hex(&spki_hash)
                    .map_err(|_| ParseGroupIdError::WrongSliceLength)?;
                Ok(Self::DeviceMetaGroup(spki_hash))
            }
            b"global" => Ok(Self::GlobalGroup),
            _ => Err(ParseGroupIdError::UnknownGroupType),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseGroupIdError {
    #[error("wrong group id length")]
    WrongSliceLength,
    #[error("unknown group type")]
    UnknownGroupType,
    #[error("missing data")]
    MissingData,
    #[error("missing group type")]
    MissingGroupType,
}

#[cfg(test)]
mod tests {
    use ring::rand::{SecureRandom, SystemRandom};

    use super::*;

    #[test]
    fn test_group_id() {
        let mut raw = [0u8; 32];
        // fill with random data
        let rand = SystemRandom::new();
        rand.fill(&mut raw).unwrap();

        let group_id = SvalinGroupId::DeviceGroup(SpkiHash::from_slice(&raw).unwrap());
        let encoded = group_id.to_group_id();
        let decoded = SvalinGroupId::from_group_id(&encoded).unwrap();
        assert_eq!(group_id, decoded);
    }
}
