use std::{collections::HashSet, marker::PhantomData};

use anyhow::{Context, anyhow};
use openmls::{
    error::LibraryError,
    prelude::{PublicMessageIn, Welcome},
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
    ) -> anyhow::Result<MessageData<Types>> {
        tracing::debug!("handling message: {:?}", message);
        match message
            .unpack()
            .map_err(|err| {
                tracing::error!("unpack error: {err}");
                anyhow!("{}", err)
            })
            .context("unpack error")?
        {
            MessageToMember::Welcome(welcome) => {
                tracing::debug!("handling welcome");
                let group = self
                    .handle_welcome(welcome)
                    .await
                    .map_err(|err| {
                        tracing::error!("error handling welcome: {}", err);
                        anyhow!("{}", err)
                    })
                    .context("handle welcome error")?;
                tracing::debug!("welcome handled successfully");

                Ok(MessageData {
                    content: MessageDataContent::Internal,
                    group,
                })
            }
            MessageToMember::GroupMessage(message) => {
                tracing::debug!("handling group message");
                let processed = self
                    .harness
                    .processor()
                    .process_message(message)
                    .await
                    .context("error processing group message")?;
                tracing::debug!("message processed successfully");
                let group_id = SvalinGroupId::from_group_id(&processed.group_id)
                    .context("error parsing group id")?;
                tracing::debug!("group id parsed successfully");
                let ProcessedContent::Message(decrypted) = processed.content else {
                    anyhow::bail!("expected data message, got something else instead.")
                };
                let decoded: SvalinMessage<Types> =
                    postcard::from_bytes(&decrypted).context("postcard error")?;

                match decoded {
                    SvalinMessage::Report(report) => match group_id.clone() {
                        SvalinGroupId::DeviceGroup(device) => {
                            if device != processed.sender {
                                anyhow::bail!(
                                    "only the device itself can send reports to its group"
                                )
                            } else {
                                Ok(MessageData {
                                    group: group_id,
                                    content: MessageDataContent::Report(device, report),
                                })
                            }
                        }
                        #[allow(unreachable_patterns)]
                        _ => anyhow::bail!("unallowed message type"),
                    },
                    SvalinMessage::MetaInfo(meta_info) => match group_id.clone() {
                        SvalinGroupId::DeviceMetaGroup(device) => Ok(MessageData {
                            group: group_id,
                            content: MessageDataContent::MetaInfo(device, meta_info),
                        }),
                        #[allow(unreachable_patterns)]
                        _ => anyhow::bail!("unallowed message type"),
                    },
                }
            }
            MessageToMember::AddToGroup(message) => self
                .handle_add_to_group(message)
                .await
                .context("add to group error"),
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

        Ok(id)
    }

    async fn handle_add_to_group(
        &self,
        message: PublicMessageIn,
    ) -> anyhow::Result<MessageData<Types>> {
        tracing::debug!("Handling add to group message");

        let processed = self
            .harness
            .processor()
            .process_message(message)
            .await
            .map_err(|err| anyhow!(err))?;
        let group_id = processed.group_id()?;

        let ProcessedContent::Commit(commit) = processed.content else {
            anyhow::bail!("Expected a commit message, got {:?}", processed.content)
        };

        self.harness.check_commit(&group_id, &commit).await?;

        self.harness.processor().commit(commit).await?;

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
        spki_hash: &SpkiHash,
    ) -> anyhow::Result<bool> {
        self.harness
            .processor()
            .list_members(group_id.to_group_id())
            .await
            .map(|members| members.contains(spki_hash))
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
        metainfo: Types::MetaInfo,
    ) -> anyhow::Result<MessageToServerTransport> {
        let group_id = SvalinGroupId::DeviceMetaGroup(self.me.clone()).to_group_id();
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
    #[error("incorrect certificate type")]
    IncorrectCertificateType,
}
