use std::collections::HashSet;

use anyhow::{Context, anyhow};
use openmls::{
    error::LibraryError,
    prelude::{PublicMessageIn, Welcome},
};
use serde::de::DeserializeOwned;

use crate::{
    CertificateType, Credential, SpkiHash, VerifyError, certificate, get_current_timestamp,
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
            DeviceMessage, MessageToMember, MessageToMemberTransport, MessageToServerTransport,
        },
    },
};

pub struct MlsClient<KeyRetriever, Verifier> {
    me: SpkiHash,
    harness: MlsHarness<KeyRetriever, Verifier, MlsProcessorHandle>,
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
            harness: MlsHarness::new(key_retriever, verifier, processor),
        })
    }

    pub fn me(&self) -> &SpkiHash {
        &self.me
    }

    pub async fn handle_message<Report: DeserializeOwned>(
        &self,
        message: &MessageToMemberTransport,
    ) -> anyhow::Result<MessageData<Report>> {
        match message.unpack().context("unpack error")? {
            MessageToMember::Welcome(welcome) => {
                let group = self
                    .handle_welcome(welcome)
                    .await
                    .map_err(|err| anyhow!(err))
                    .context("handle welcome error")?;

                Ok(MessageData {
                    content: MessageDataContent::Internal,
                    group,
                })
            }
            MessageToMember::GroupMessage(message) => {
                let processed = self.harness.processor().process_message(message).await?;
                let group_id = SvalinGroupId::from_group_id(&processed.group_id)?;
                let ProcessedContent::Message(decrypted) = processed.content else {
                    anyhow::bail!("expected data message, got something else instead.")
                };
                let decoded: DeviceMessage<Report> = postcard::from_bytes(&decrypted)?;

                match decoded {
                    DeviceMessage::Report(report) => match group_id.clone() {
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

        self.harness.processor().join_group(staged).await?;

        Ok(id)
    }

    async fn handle_add_to_group<Report>(
        &self,
        message: PublicMessageIn,
    ) -> anyhow::Result<MessageData<Report>> {
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
