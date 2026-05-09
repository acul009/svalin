use anyhow::anyhow;
use openmls::{error::LibraryError, prelude::PublicMessageIn};
use openmls_sqlx_storage::SqliteStorageProvider;
use serde::Serialize;
use tokio::task::JoinError;

use crate::{
    Certificate, CertificateType, Credential, VerifyError,
    mls::{
        SvalinGroupId,
        group_id::ParseGroupIdError,
        harness::MlsHarness,
        key_package::{KeyPackage, KeyPackageError},
        processor::{
            CreateGroupError, CreateGroupMessageError, CreateKeyPackageError, GroupExistsError,
            JoinGroupError, MlsProcessorHandle, ProcessedContent,
        },
        provider::{PostcardCodec, SvalinProvider},
        transport_types::{
            DeviceMessage, MessageToMember, MessageToMemberTransport, MessageToServerTransport,
        },
    },
};

pub struct MlsAgent<KeyRetriever, Verifier> {
    me: Certificate,
    my_device_group: SvalinGroupId,
    harness: MlsHarness<KeyRetriever, Verifier, MlsProcessorHandle>,
}

#[derive(Debug, thiserror::Error)]
pub enum MlsAgentCreateError {
    #[error("given certificate is not an agent: {0:?}")]
    NotAnAgent(Certificate),
    #[error("storage error: {0}")]
    StorageError(<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError),
    #[error("this agent does not know about his own group")]
    MissingMyGroup,
    #[error("join error: {0}")]
    JoinError(#[from] JoinError),
}

impl<KeyRetriever, Verifier> MlsAgent<KeyRetriever, Verifier>
where
    KeyRetriever: crate::mls::key_retriever::KeyRetriever,
    Verifier: crate::Verifier,
{
    pub async fn new(
        credential: Credential,
        storage_provider: SqliteStorageProvider<PostcardCodec>,
        key_retriever: KeyRetriever,
        verifier: Verifier,
    ) -> Result<Self, MlsAgentCreateError> {
        let me = credential.certificate().clone();
        if credential.certificate().certificate_type() != CertificateType::Agent {
            return Err(MlsAgentCreateError::NotAnAgent(
                credential.certificate().clone(),
            ));
        }

        let processor = MlsProcessorHandle::new_processor(credential, storage_provider.into());

        Ok(Self {
            my_device_group: SvalinGroupId::DeviceGroup(me.spki_hash().clone()),
            me,
            harness: MlsHarness::new(key_retriever, verifier, processor),
        })
    }

    pub async fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        self.harness.processor().create_key_package().await
    }

    pub async fn handle_message(
        &self,
        message: &MessageToMemberTransport,
    ) -> Result<(), anyhow::Error> {
        let message = message.unpack()?;

        match message {
            MessageToMember::Welcome(_welcome) => {
                todo!("Don't have a use case for an agent joining another group yet")
            }
            MessageToMember::GroupMessage(_private_message_in) => {
                todo!("There aren't any reasons for an agent to receive a message yet")
            }
            MessageToMember::AddToGroup(message) => self.handle_add_to_group(message).await,
        }
    }

    async fn handle_add_to_group(&self, message: PublicMessageIn) -> Result<(), anyhow::Error> {
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

        if group_id != self.my_device_group {
            anyhow::bail!("received message for unexpected group: {:?}", group_id)
        }

        self.harness.check_commit(&group_id, &commit).await?;

        self.harness.processor().commit(commit).await?;

        Ok(())
    }

    // async fn handle_welcome(
    //     &self,
    //     welcome: Welcome,
    // ) -> Result<(), HandleWelcomeError<KeyRetriever::Error>> {
    //     let staged = self.harness.processor().stage_join(welcome).await?;
    //     let id = SvalinGroupId::from_group_id(staged.group_context().group_id())?;

    //     match &id {
    //         SvalinGroupId::DeviceGroup(device) => {
    //             if device != self.me.spki_hash() {
    //                 return Err(HandleWelcomeError::UnwantedGroup);
    //             }
    //         }
    //     }

    //     let required_members = self
    //         .harness.key_retriever()    //         .get_required_group_members(&id)
    //         .await
    //         .map_err(HandleWelcomeError::RetrieverError)?
    //         .into_iter()
    //         .collect::<HashSet<_>>();

    //     let members = staged
    //         .members()
    //         .map(|m| m.credential.deserialized())
    //         .collect::<Result<HashSet<SpkiHash>, tls_codec::Error>>()?;

    //     if members != required_members {
    //         return Err(HandleWelcomeError::IncorrectMembers);
    //     }

    //     self.harness.processor().join_group(staged).await?;

    //     Ok(())
    // }

    pub async fn send_report<Report: Serialize>(
        &self,
        report: Report,
    ) -> Result<MessageToServerTransport, SendDeviceMessageError> {
        let group_id = SvalinGroupId::DeviceGroup(self.me.spki_hash().clone()).to_group_id();
        let message = DeviceMessage::Report(report);
        let encoded = postcard::to_stdvec(&message)?;
        let to_server = self
            .harness
            .processor()
            .create_message(group_id, encoded)
            .await?;

        Ok(to_server)
    }

    pub async fn create_device_group_if_missing(
        &self,
    ) -> Result<Option<MessageToServerTransport>, CreateSvalinGroupError<KeyRetriever::Error>> {
        let group_id = SvalinGroupId::DeviceGroup(self.me.spki_hash().clone());

        self.harness
            .create_group_if_not_exists(&group_id, self.me.spki_hash())
            .await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateSystemreportError {
    #[error("postcard error: {0}")]
    PostcardError(#[from] postcard::Error),
    #[error("create message error: {0}")]
    CreateMessageError(#[from] CreateGroupMessageError),
}

#[derive(Debug, thiserror::Error)]
pub enum HandleMessageError<RetrieverError> {
    #[error("welcome error: {0}")]
    Welcome(#[from] HandleWelcomeError<RetrieverError>),
    #[error("tls codec error: {0}")]
    TlsCodex(#[from] tls_codec::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum HandleWelcomeError<RetrieverError> {
    #[error("join group error: {0}")]
    JoinGroupError(#[from] JoinGroupError),
    #[error("parse group id error: {0}")]
    ParseGroupIdError(#[from] ParseGroupIdError),
    #[error("unwanted group")]
    UnwantedGroup,
    #[error("retriever error: {0}")]
    RetrieverError(#[source] RetrieverError),
    #[error("tls codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("incorrect members")]
    IncorrectMembers,
    #[error("library error: {0}")]
    LibraryError(#[from] LibraryError),
}

#[derive(Debug, thiserror::Error)]
pub enum SendDeviceMessageError {
    #[error("postcard error: {0}")]
    PostcardError(#[from] postcard::Error),
    #[error("create message error: {0}")]
    CreateMessageError(#[from] CreateGroupMessageError),
}

#[derive(Debug, thiserror::Error)]
pub enum CreateSvalinGroupError<KeyRetrieverError> {
    #[error("wrong certificate type: {0}, expected {1}")]
    WrongCertificateType(CertificateType, CertificateType),
    #[error("error creating mls group: {0}")]
    CreateGroupError(#[from] CreateGroupError),
    #[error("error during key retrieval: {0}")]
    KeyRetrieverError(#[source] KeyRetrieverError),
    #[error("error verifying key package: {0}")]
    KeyPackageError(#[from] KeyPackageError),
    #[error("error verifying spki hash: {0}")]
    VerifyError(#[from] VerifyError),
    #[error("error while checking if group exists: {0}")]
    GroupExistsError(#[from] GroupExistsError),
}
