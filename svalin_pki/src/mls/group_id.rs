use openmls::group::GroupId;

use crate::SpkiHash;

pub enum SvalinGroupId {
    DeviceGroup(SpkiHash),
}

impl SvalinGroupId {
    pub(crate) fn to_group_id(&self) -> GroupId {
        match self {
            SvalinGroupId::DeviceGroup(spki_hash) => {
                let mut bytes = b"device/".to_vec();
                bytes.extend_from_slice(spki_hash.as_slice());
                GroupId::from_slice(&bytes)
            }
        }
    }

    pub(crate) fn from_group_id(group_id: &GroupId) -> Result<Self, ParseGroupIdError> {
        if group_id.as_slice().starts_with(b"device/") {
            if group_id.as_slice().len() != 39 {
                return Err(ParseGroupIdError::WrongSliceLength);
            }
            let raw_spki_hash: [u8; 32] = group_id.as_slice()[7..39]
                .try_into()
                .expect("already checked length");
            Ok(Self::DeviceGroup(SpkiHash(raw_spki_hash)))
        } else {
            Err(ParseGroupIdError::UnknownGroupType)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseGroupIdError {
    #[error("wrong group id length")]
    WrongSliceLength,
    #[error("unknown group type")]
    UnknownGroupType,
}
