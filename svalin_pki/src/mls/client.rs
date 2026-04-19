use std::collections::HashSet;

use openmls::{
    error::LibraryError,
    prelude::{ProtocolVersion, Welcome},
};
use openmls_rust_crypto::RustCrypto;
use serde::de::DeserializeOwned;

use crate::{
    CertificateType, Credential, SpkiHash, VerifyError, get_current_timestamp,
    mls::{
        group_id::{ParseGroupIdError, SvalinGroupId},
        key_package::KeyPackage,
        processor::{
            CreateKeyPackageError, JoinGroupError, MlsProcessorHandle, ProcessMessageError,
        },
        provider::SvalinStorage,
        transport_types::{DeviceMessage, MessageToMember, MessageToMemberTransport},
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

pub struct MessageData<Report> {
    pub group: SvalinGroupId,
    pub content: MessageDataContent<Report>,
}

pub enum MessageDataContent<Report> {
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
        storage_provider: SvalinStorage,
        key_retriever: KeyRetriever,
        verifier: Verifier,
    ) -> Result<Self, CreateClientError> {
        let me = credential.certificate().spki_hash().clone();
        match credential.certificate().certificate_type() {
            crate::CertificateType::Root => (),
            crate::CertificateType::User => (),
            crate::CertificateType::UserSession => (),
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

    pub fn me(&self) -> &SpkiHash {
        &self.me
    }

    pub async fn handle_message<Report: DeserializeOwned>(
        &self,
        message: &MessageToMemberTransport,
    ) -> Result<MessageData<Report>, HandleMessageError<KeyRetriever::Error>> {
        match message.unpack()? {
            MessageToMember::Welcome(welcome) => {
                let group = self.handle_welcome(welcome).await?;

                Ok(MessageData {
                    content: MessageDataContent::Internal,
                    group,
                })
            }
            MessageToMember::GroupMessage(message) => {
                let processed = self.processor.process_message(message).await?;
                let group_id = SvalinGroupId::from_group_id(&processed.group_id)?;
                let decoded: DeviceMessage<Report> = postcard::from_bytes(&processed.decrypted)?;

                match decoded {
                    DeviceMessage::Report(report) => match group_id.clone() {
                        SvalinGroupId::DeviceGroup(device) => {
                            if device != processed.sender {
                                Err(HandleMessageError::ForbiddenSender)
                            } else {
                                Ok(MessageData {
                                    group: group_id,
                                    content: MessageDataContent::Report(device, report),
                                })
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
    ) -> Result<SvalinGroupId, HandleWelcomeError<KeyRetriever::Error>> {
        let staged = self.processor.stage_join(welcome).await?;
        let id = SvalinGroupId::from_group_id(staged.group_context().group_id())?;

        match &id {
            SvalinGroupId::DeviceGroup(device) => {
                self.verifier
                    .verify_spki_hash(device, get_current_timestamp())
                    .await?;
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

        self.processor.join_group(staged).await?;

        Ok(id)
    }

    pub async fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        self.processor.create_key_package().await
    }

    pub async fn is_member(
        &self,
        group_id: &SvalinGroupId,
        spki_hash: &SpkiHash,
    ) -> Result<bool, anyhow::Error> {
        self.processor
            .list_members(group_id.to_group_id())
            .await
            .map(|members| members.contains(spki_hash))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateClientError {
    #[error("wrong certificate type: {0}, expected root, user or userdevice")]
    WrongCertificateType(CertificateType),
}

#[derive(Debug, thiserror::Error)]
pub enum HandleMessageError<RetrieverError> {
    #[error("tls codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
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
    #[error("verify error: {0}")]
    VerifyError(#[from] VerifyError),
}
