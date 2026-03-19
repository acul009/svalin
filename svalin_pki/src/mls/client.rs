use std::marker::PhantomData;

use openmls::{
    group::GroupId,
    prelude::{MlsMessageOut, Welcome, group_info::GroupInfo},
};
use openmls_sqlx_storage::SqliteStorageProvider;

use crate::{
    Certificate, CertificateType, Credential, SpkiHash,
    mls::{
        processor::{CreateGroupError, MlsProcessorHandle},
        provider::PostcardCodec,
        transport_types::MessageToServerTransport,
    },
};

pub struct MlsClient {
    processor: MlsProcessorHandle,
}

impl MlsClient {
    pub fn new(
        credential: Credential,
        storage_provider: SqliteStorageProvider<PostcardCodec>,
    ) -> Result<Self, CreateClientError> {
        match credential.certificate().certificate_type() {
            crate::CertificateType::Root => (),
            crate::CertificateType::User => (),
            crate::CertificateType::UserDevice => (),
            cert_type => return Err(CreateClientError::WrongCertificateType(cert_type)),
        }

        let processor = MlsProcessorHandle::new_processor(credential, storage_provider);

        Ok(Self { processor })
    }

    pub async fn create_device_group(
        &self,
        device: Certificate,
    ) -> Result<MessageToServerTransport, CreateDeviceGroupError> {
        if device.certificate_type() != CertificateType::Agent {
            return Err(CreateDeviceGroupError::WrongCertificateType(
                device.certificate_type(),
            ));
        }

        let group_id = SvalinGroupId::DeviceGroup(device.spki_hash().clone());

        let members = todo!();

        let messages = self
            .processor
            .create_group(members, group_id.to_group_id())
            .await?;

        Ok(messages)
    }
}

pub(crate) enum SvalinGroupId {
    DeviceGroup(SpkiHash),
}

impl SvalinGroupId {
    pub fn to_group_id(&self) -> GroupId {
        match self {
            SvalinGroupId::DeviceGroup(spki_hash) => {
                let mut bytes = b"device/".to_vec();
                bytes.extend_from_slice(spki_hash.as_slice());
                GroupId::from_slice(&bytes)
            }
        }
    }

    pub fn from_group_id(group_id: &GroupId) -> Result<Self, ParseGroupIdError> {
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

#[derive(Debug, thiserror::Error)]
pub enum CreateClientError {
    #[error("wrong certificate type: {0}, expected root, user or userdevice")]
    WrongCertificateType(CertificateType),
}

#[derive(Debug, thiserror::Error)]
pub enum CreateDeviceGroupError {
    #[error("wrong certificate type: {0}, expected agent")]
    WrongCertificateType(CertificateType),
    #[error("error creating mls group: {0}")]
    CreateGroupError(#[from] CreateGroupError),
}
