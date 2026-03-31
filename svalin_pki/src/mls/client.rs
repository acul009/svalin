use std::{collections::HashSet, marker::PhantomData};

use openmls::{
    error::LibraryError,
    group::GroupId,
    prelude::{MlsMessageOut, OpenMlsCrypto, ProtocolVersion, Welcome, group_info::GroupInfo},
};
use openmls_rust_crypto::RustCrypto;
use openmls_sqlx_storage::SqliteStorageProvider;
use serde::de::DeserializeOwned;

use crate::{
    Certificate, CertificateType, Credential, ExactVerififier, SpkiHash,
    mls::{
        group_id::{ParseGroupIdError, SvalinGroupId},
        key_package::{self, KeyPackage, KeyPackageError, UnverifiedKeyPackage},
        key_retriever::{self, KeyRetriever},
        processor::{
            self, CreateGroupError, CreateKeyPackageError, JoinGroupError, MlsProcessorHandle,
            ProcessMessageError,
        },
        provider::PostcardCodec,
        transport_types::{
            DeviceMessage, MessageToMember, MessageToServer, NewGroup, NewGroupTransport,
        },
    },
};

pub struct MlsClient<KeyRetriever, Verifier> {
    me: SpkiHash,
    processor: MlsProcessorHandle,
    key_retriever: KeyRetriever,
    verifier: Verifier,
    crypto: RustCrypto,
    protocol_version: ProtocolVersion,
}

pub enum MessageData<Report> {
    Report(SpkiHash, Report),
    Internal,
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
        let me = credential.certificate().spki_hash().clone();
        match credential.certificate().certificate_type() {
            crate::CertificateType::Root => (),
            crate::CertificateType::User => (),
            crate::CertificateType::UserDevice => (),
            cert_type => return Err(CreateClientError::WrongCertificateType(cert_type)),
        }

        let processor = MlsProcessorHandle::new_processor(credential, storage_provider);

        Ok(Self {
            me,
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
        key_package: UnverifiedKeyPackage,
    ) -> Result<NewGroupTransport, CreateDeviceGroupError<KeyRetriever::Error>> {
        let verifier = ExactVerififier::new(device);
        let key_package = key_package
            .verify(&self.crypto, self.protocol_version, &verifier)
            .await?;
        let device = key_package.certificate();
        if device.certificate_type() != CertificateType::Agent {
            return Err(CreateDeviceGroupError::WrongCertificateType(
                device.certificate_type(),
            ));
        }

        let group_id = SvalinGroupId::DeviceGroup(device.spki_hash().clone());

        let required_members = self
            .key_retriever
            .get_required_group_members(&group_id)
            .await
            .map_err(CreateDeviceGroupError::KeyRetrieverError)?
            .into_iter()
            .filter(|spki_hash| spki_hash != &self.me && spki_hash != device.spki_hash())
            .collect::<Vec<_>>();

        let unverified = self
            .key_retriever
            .get_key_packages(&required_members)
            .await
            .map_err(CreateDeviceGroupError::KeyRetrieverError)?;

        let mut members = Vec::with_capacity(unverified.len() + 1);
        members.push(key_package);
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

    pub async fn handle_message<Report: DeserializeOwned>(
        &self,
        message: MessageToMember,
    ) -> Result<MessageData<Report>, HandleMessageError<KeyRetriever::Error>> {
        match message {
            MessageToMember::Welcome(welcome) => {
                self.handle_welcome(welcome).await?;

                Ok(MessageData::Internal)
            }
            MessageToMember::GroupMessage(message) => {
                let processed = self.processor.process_message(message).await?;
                let group_id = SvalinGroupId::from_group_id(&processed.group_id)?;
                let decoded: DeviceMessage<Report> = postcard::from_bytes(&processed.decrypted)?;

                match decoded {
                    DeviceMessage::Report(report) => match group_id {
                        SvalinGroupId::DeviceGroup(device) => {
                            if device != processed.sender {
                                Err(HandleMessageError::ForbiddenSender)
                            } else {
                                Ok(MessageData::Report(device, report))
                            }
                        }
                        #[allow(unreachable_patterns)]
                        _ => return Err(HandleMessageError::InvalidMessage),
                    },
                }
            }
        }
    }

    async fn handle_welcome(
        &self,
        welcome: Welcome,
    ) -> Result<(), HandleWelcomeError<KeyRetriever::Error>> {
        let staged = self.processor.stage_join(welcome).await?;
        let id = SvalinGroupId::from_group_id(staged.group_context().group_id())?;

        match &id {
            SvalinGroupId::DeviceGroup(_device) => {
                // No additional verification for now
            }
        }

        let required_members = self
            .key_retriever
            .get_required_group_members(&id)
            .await
            .map_err(HandleWelcomeError::RetrieverError)?
            .into_iter()
            .collect::<HashSet<_>>();

        let members = staged
            .members()
            .map(|m| m.credential.deserialized())
            .collect::<Result<HashSet<SpkiHash>, tls_codec::Error>>()?;

        if members != required_members {
            return Err(HandleWelcomeError::IncorrectMembers);
        }

        Ok(())
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

#[derive(Debug, thiserror::Error)]
pub enum HandleMessageError<RetrieverError> {
    #[error("welcome error: {0}")]
    Welcome(#[from] HandleWelcomeError<RetrieverError>),
    #[error("process message error: {0}")]
    ProcessMessage(#[from] ProcessMessageError),
    #[error("deserialize error: {0}")]
    DeserializeError(#[from] postcard::Error),
    #[error("group id error: {0}")]
    GroupIdError(#[from] ParseGroupIdError),
    #[error("invalid message")]
    InvalidMessage,
    #[error("forbidden sender")]
    ForbiddenSender,
}

#[derive(Debug, thiserror::Error)]
pub enum HandleWelcomeError<RetrieverError> {
    #[error("join group error: {0}")]
    JoinGroupError(#[from] JoinGroupError),
    #[error("parse group id error: {0}")]
    ParseGroupIdError(#[from] ParseGroupIdError),
    #[error("retriever error: {0}")]
    RetrieverError(#[source] RetrieverError),
    #[error("tls codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("incorrect members")]
    IncorrectMembers,
    #[error("library error: {0}")]
    LibraryError(#[from] LibraryError),
}
