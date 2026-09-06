use std::{collections::HashSet, marker::PhantomData};

use anyhow::anyhow;
use openmls::{
    error::LibraryError,
    framing::ProtocolMessage,
    prelude::{PublicMessageIn, Welcome, tls_codec},
};

use crate::{
    CertificateType, Credential, SpkiHash, VerifyError, get_current_timestamp,
    mls::{
        group_id::{ParseGroupIdError, SvalinGroupId},
        harness::MlsHarness,
        key_package::KeyPackage,
        processor::{
            CreateKeyPackageError, JoinGroupError, MlsProcessorHandle, ProcessMessageError,
            ProcessedContent,
        },
        provider::SvalinStorage,
        transport_types::{
            MessageToMember, MessageToMemberTransport, MessageToServerTransport, MessageTypes,
            SvalinMessage,
        },
    },
};

pub struct MlsClient<Types: MessageTypes, KeyRetriever, Verifier> {
    me: SpkiHash,
    harness: MlsHarness<KeyRetriever, Verifier, MlsProcessorHandle>,
    _types: PhantomData<Types>,
}

pub struct MessageData<Types: MessageTypes> {
    pub group: SvalinGroupId,
    pub content: MessageDataContent<Types>,
}

pub enum MessageDataContent<Types: MessageTypes> {
    Report(SpkiHash, Types::Report),
    MetaInfo(SpkiHash, Types::MetaInfo),
    Internal,
}

impl<Types, KeyRetriever, Verifier> MlsClient<Types, KeyRetriever, Verifier>
where
    KeyRetriever: crate::mls::key_retriever::KeyRetriever,
    Verifier: crate::Verifier,
    Types: MessageTypes,
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
            harness: MlsHarness::new(key_retriever, verifier, processor),
            _types: PhantomData,
        })
    }

    pub fn me(&self) -> &SpkiHash {
        &self.me
    }

    pub async fn handle_message(
        &self,
        message: &MessageToMemberTransport,
    ) -> Result<MessageData<Types>, HandleMessageError<KeyRetriever::Error>> {
        tracing::trace!("handling message: {:?}", message);
        match message.unpack()? {
            MessageToMember::Welcome(welcome) => {
                tracing::trace!("handling welcome");
                let group = self.handle_welcome(welcome).await?;
                tracing::trace!("welcome handled successfully");

                Ok(MessageData {
                    content: MessageDataContent::Internal,
                    group,
                })
            }
            MessageToMember::GroupMessage(message) => {
                let message = ProtocolMessage::PrivateMessage(message);
                let group_id = SvalinGroupId::from_group_id(message.group_id())?;
                let ProtocolMessage::PrivateMessage(message) = message else {
                    unreachable!()
                };
                tracing::trace!("handling group message");
                let processed = match self.harness.processor().process_message(message).await {
                    Ok(processed) => processed,
                    Err(err) => match err {
                        ProcessMessageError::ProcessError(
                            openmls::group::ProcessMessageError::ValidationError(
                                openmls::group::ValidationError::CannotDecryptOwnMessage,
                            ),
                        ) => {
                            return Ok(MessageData {
                                content: MessageDataContent::Internal,
                                group: group_id,
                            });
                        }
                        source => {
                            return Err(HandleMessageError::ProcessMessage { group_id, source });
                        }
                    },
                };
                tracing::trace!("message processed successfully");
                tracing::trace!("group id parsed successfully");
                let ProcessedContent::Message(decrypted) = processed.content else {
                    return Err(HandleMessageError::InvalidMessage { group_id });
                };
                let decoded: SvalinMessage<Types> =
                    postcard::from_bytes(&decrypted).map_err(|source| {
                        HandleMessageError::Deserialize {
                            group_id: group_id.clone(),
                            source,
                        }
                    })?;

                match decoded {
                    SvalinMessage::Report(report) => match group_id.clone() {
                        SvalinGroupId::DeviceGroup(device) => {
                            if device != processed.sender {
                                Err(HandleMessageError::ForbiddenSender { group_id })
                            } else {
                                Ok(MessageData {
                                    group: group_id,
                                    content: MessageDataContent::Report(device, report),
                                })
                            }
                        }
                        #[allow(unreachable_patterns)]
                        _ => Err(HandleMessageError::InvalidMessage { group_id }),
                    },
                    SvalinMessage::MetaInfo(meta_info) => match group_id.clone() {
                        SvalinGroupId::DeviceMetaGroup(device) => Ok(MessageData {
                            group: group_id,
                            content: MessageDataContent::MetaInfo(device, meta_info),
                        }),
                        #[allow(unreachable_patterns)]
                        _ => Err(HandleMessageError::InvalidMessage { group_id }),
                    },
                }
            }
            MessageToMember::AddToGroup(message) => self.handle_add_to_group(message).await,
        }
    }

    async fn handle_welcome(
        &self,
        welcome: Welcome,
    ) -> Result<SvalinGroupId, HandleWelcomeError<KeyRetriever::Error>> {
        let staged = self.harness.processor().stage_join(welcome).await?;
        let id = SvalinGroupId::from_group_id(staged.group_context().group_id())?;

        match &id {
            SvalinGroupId::DeviceGroup(spki_hash) => {
                let certificate = self
                    .harness
                    .verifier()
                    .verify_spki_hash(spki_hash, get_current_timestamp())
                    .await?;

                if certificate.certificate_type() != CertificateType::Agent {
                    return Err(HandleWelcomeError::IncorrectCertificateType);
                }
                // No additional verification for now
            }
            SvalinGroupId::DeviceMetaGroup(spki_hash) => {
                let certificate = self
                    .harness
                    .verifier()
                    .verify_spki_hash(spki_hash, get_current_timestamp())
                    .await?;

                if certificate.certificate_type() != CertificateType::Agent {
                    return Err(HandleWelcomeError::IncorrectCertificateType);
                }
            }
        }

        let required_members = self
            .harness
            .key_retriever()
            .get_required_group_members(&id)
            .await
            .map_err(HandleWelcomeError::RetrieverError)?;

        let members = staged
            .members()
            .map(|m| m.credential.deserialized())
            .collect::<Result<HashSet<SpkiHash>, tls_codec::Error>>()?;

        for required in required_members {
            if !members.contains(&required) {
                return Err(HandleWelcomeError::IncorrectMembers);
            }
        }

        self.harness.processor().join_group(staged).await?;

        tracing::trace!("joined group {id:?}");

        Ok(id)
    }

    async fn handle_add_to_group(
        &self,
        message: PublicMessageIn,
    ) -> Result<MessageData<Types>, HandleMessageError<KeyRetriever::Error>> {
        tracing::trace!("Handling add to group message");

        let message = ProtocolMessage::PublicMessage(Box::new(message));
        let group_id = SvalinGroupId::from_group_id(message.group_id())?;
        let ProtocolMessage::PublicMessage(message) = message else {
            unreachable!()
        };
        let message = *message;
        let processed = self
            .harness
            .processor()
            .process_message(message)
            .await
            .map_err(|source| HandleMessageError::ProcessMessage {
                group_id: group_id.clone(),
                source,
            })?;

        let ProcessedContent::Commit(commit) = processed.content else {
            return Err(HandleMessageError::InvalidMessage { group_id });
        };

        self.harness
            .check_commit(&group_id, &commit)
            .await
            .map_err(|source| HandleMessageError::CheckCommit {
                group_id: group_id.clone(),
                source,
            })?;

        self.harness
            .processor()
            .commit(commit)
            .await
            .map_err(|source| HandleMessageError::Commit {
                group_id: group_id.clone(),
                source,
            })?;

        Ok(MessageData {
            group: group_id,
            content: MessageDataContent::Internal,
        })
    }

    pub async fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        self.harness.processor().create_key_package().await
    }

    pub async fn is_member(
        &self,
        group_id: &SvalinGroupId,
        spki_hash: SpkiHash,
    ) -> anyhow::Result<bool> {
        self.harness
            .processor()
            .is_member(group_id.to_group_id(), spki_hash)
            .await
    }

    pub async fn add_member(
        &self,
        group: &SvalinGroupId,
        key_package: KeyPackage,
    ) -> anyhow::Result<MessageToServerTransport> {
        let message_to_server = self
            .harness
            .processor()
            .add_member(group.to_group_id(), key_package)
            .await?;

        Ok(message_to_server)
    }

    pub async fn create_meta_group_if_missing(
        &self,
        spki_hash: SpkiHash,
    ) -> anyhow::Result<Option<MessageToServerTransport>> {
        let certificate = self
            .harness
            .verifier()
            .verify_spki_hash(&spki_hash, get_current_timestamp())
            .await?;
        if certificate.certificate_type() != CertificateType::Agent {
            anyhow::bail!("wrong spki hash type");
        }
        let group_id = SvalinGroupId::DeviceMetaGroup(spki_hash);

        Ok(self
            .harness
            .create_group_if_not_exists(&group_id, &self.me)
            .await
            .map_err(|err| anyhow!(err))?)
    }

    pub async fn send_meta_info(
        &self,
        spki_hash: SpkiHash,
        metainfo: Types::MetaInfo,
    ) -> anyhow::Result<MessageToServerTransport> {
        let group_id = SvalinGroupId::DeviceMetaGroup(spki_hash).to_group_id();
        let message = SvalinMessage::<Types>::MetaInfo(metainfo);
        let encoded = postcard::to_stdvec(&message)?;
        let to_server = self
            .harness
            .processor()
            .create_message(group_id, encoded)
            .await?;

        Ok(to_server)
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
    #[error("error processing message for group {group_id:?}: {source}")]
    ProcessMessage {
        group_id: SvalinGroupId,
        #[source]
        source: ProcessMessageError,
    },
    #[error("error deserializing message for group {group_id:?}: {source}")]
    Deserialize {
        group_id: SvalinGroupId,
        #[source]
        source: postcard::Error,
    },
    #[error("group id error: {0}")]
    GroupIdError(#[from] ParseGroupIdError),
    #[error("invalid message for group {group_id:?}")]
    InvalidMessage { group_id: SvalinGroupId },
    #[error("forbidden sender for group {group_id:?}")]
    ForbiddenSender { group_id: SvalinGroupId },
    #[error("commit validation error for group {group_id:?}: {source}")]
    CheckCommit {
        group_id: SvalinGroupId,
        #[source]
        source: anyhow::Error,
    },
    #[error("commit error for group {group_id:?}: {source}")]
    Commit {
        group_id: SvalinGroupId,
        #[source]
        source: anyhow::Error,
    },
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
    #[error("incorrect certificate type")]
    IncorrectCertificateType,
}
