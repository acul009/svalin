use std::marker::PhantomData;

use openmls::{
    group::GroupId,
    prelude::{MlsMessageOut, ProtocolVersion, Welcome, group_info::GroupInfo},
};
use openmls_rust_crypto::RustCrypto;
use openmls_sqlx_storage::SqliteStorageProvider;

use crate::{
    Certificate, CertificateType, Credential, SpkiHash,
    mls::{
        group_id::SvalinGroupId,
        key_package::{KeyPackage, KeyPackageError},
        key_retriever::{self, KeyRetriever},
        processor::{CreateGroupError, CreateKeyPackageError, MlsProcessorHandle},
        provider::PostcardCodec,
        transport_types::{MessageToServer, NewGroup, NewGroupTransport},
    },
};

pub struct MlsClient<KeyRetriever, Verifier> {
    processor: MlsProcessorHandle,
    key_retriever: KeyRetriever,
    verifier: Verifier,
    crypto: RustCrypto,
    protocol_version: ProtocolVersion,
}

impl<KeyRetriever, Verifier> MlsClient<KeyRetriever, Verifier>
where
    KeyRetriever: crate::mls::key_retriever::KeyRetriever,
    Verifier: crate::Verifier,
{
    pub fn new(
        credential: Credential,
        storage_provider: SqliteStorageProvider<PostcardCodec>,
        key_retriever: KeyRetriever,
        verifier: Verifier,
    ) -> Result<Self, CreateClientError> {
        match credential.certificate().certificate_type() {
            crate::CertificateType::Root => (),
            crate::CertificateType::User => (),
            crate::CertificateType::UserDevice => (),
            cert_type => return Err(CreateClientError::WrongCertificateType(cert_type)),
        }

        let processor = MlsProcessorHandle::new_processor(credential, storage_provider);

        Ok(Self {
            processor,
            key_retriever,
            verifier,
            crypto: RustCrypto::default(),
            protocol_version: ProtocolVersion::default(),
        })
    }

    pub async fn create_device_group(
        &self,
        device: Certificate,
    ) -> Result<NewGroupTransport, CreateDeviceGroupError<KeyRetriever::Error>> {
        if device.certificate_type() != CertificateType::Agent {
            return Err(CreateDeviceGroupError::WrongCertificateType(
                device.certificate_type(),
            ));
        }

        let group_id = SvalinGroupId::DeviceGroup(device.spki_hash().clone());

        let required_members = self
            .key_retriever
            .get_required_device_group_members(device.spki_hash())
            .await
            .map_err(CreateDeviceGroupError::KeyRetrieverError)?;

        let unverified = self
            .key_retriever
            .get_key_packages(&required_members)
            .await
            .map_err(CreateDeviceGroupError::KeyRetrieverError)?;

        let mut members = Vec::with_capacity(unverified.len());
        for member in unverified {
            let member = member
                .verify(&self.crypto, self.protocol_version, &self.verifier)
                .await?;
            members.push(member);
        }

        let messages = self
            .processor
            .create_group(members, group_id.to_group_id())
            .await?;

        Ok(messages)
    }

    pub async fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        self.processor.create_key_package().await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateClientError {
    #[error("wrong certificate type: {0}, expected root, user or userdevice")]
    WrongCertificateType(CertificateType),
}

#[derive(Debug, thiserror::Error)]
pub enum CreateDeviceGroupError<KeyRetrieverError> {
    #[error("wrong certificate type: {0}, expected agent")]
    WrongCertificateType(CertificateType),
    #[error("error creating mls group: {0}")]
    CreateGroupError(#[from] CreateGroupError),
    #[error("error during key retrieval: {0}")]
    KeyRetrieverError(#[source] KeyRetrieverError),
    #[error("error verifying key package: {0}")]
    KeyPackageError(#[from] KeyPackageError),
}
