use anyhow::anyhow;
use openmls::{
    error::LibraryError,
    prelude::{ProtocolVersion, PublicMessageIn},
};
use openmls_rust_crypto::RustCrypto;
use openmls_sqlx_storage::SqliteStorageProvider;
use serde::Serialize;
use tokio::task::JoinError;

use crate::{
    Certificate, CertificateType, Credential, SpkiHash, VerifyError, get_current_timestamp,
    mls::{
        SvalinGroupId,
        group_id::ParseGroupIdError,
        key_package::{KeyPackage, KeyPackageError, UnverifiedKeyPackage},
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
    processor: MlsProcessorHandle,
    key_retriever: KeyRetriever,
    verifier: Verifier,
    me: Certificate,
    my_device_group: SvalinGroupId,
    crypto: RustCrypto,
    protocol_version: ProtocolVersion,
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
            processor,
            key_retriever,
            verifier,
            crypto: RustCrypto::default(),
            protocol_version: ProtocolVersion::default(),
        })
    }

    pub async fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        self.processor.create_key_package().await
    }

    pub async fn handle_message(
        &self,
        message: MessageToMemberTransport,
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
            .processor
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

        let required = self
            .key_retriever
            .get_required_group_members(&group_id)
            .await
            .map_err(|err| anyhow!(err))?;
        for proposal in commit.remove_proposals() {
            let index = proposal.remove_proposal().removed();
            let spki_hash = self
                .processor
                .get_member(group_id.to_group_id(), index)
                .await?;
            if required.contains(&spki_hash) {
                anyhow::bail!("Cannot remove required member: {spki_hash:?}")
            }
        }

        for proposal in commit.add_proposals() {
            let raw_key_package = proposal.add_proposal().key_package();
            let key_package = UnverifiedKeyPackage::new(raw_key_package.clone().into());
            let key_package = key_package
                .verify(&self.crypto, self.protocol_version, &self.verifier)
                .await?;
            match key_package.certificate().certificate_type() {
                CertificateType::User => (),
                CertificateType::UserSession => (),
                certificate_type => anyhow::bail!(
                    "Unexpected certificate type in key package: {certificate_type:?}"
                ),
            }
        }

        for mls_credential in commit.credentials_to_verify() {
            let spki_hash: SpkiHash = mls_credential.deserialized()?;
            self.verifier
                .verify_spki_hash(&spki_hash, get_current_timestamp())
                .await?;
        }

        let verified = self
            .verifier
            .verify_spki_hash(&processed.sender, get_current_timestamp())
            .await?;

        match verified.certificate_type() {
            CertificateType::Root => (),
            CertificateType::User => (),
            _ => anyhow::bail!(
                "Sender {:?} is not allowed to add members to the group",
                processed.sender
            ),
        };

        Ok(())
    }

    // async fn handle_welcome(
    //     &self,
    //     welcome: Welcome,
    // ) -> Result<(), HandleWelcomeError<KeyRetriever::Error>> {
    //     let staged = self.processor.stage_join(welcome).await?;
    //     let id = SvalinGroupId::from_group_id(staged.group_context().group_id())?;

    //     match &id {
    //         SvalinGroupId::DeviceGroup(device) => {
    //             if device != self.me.spki_hash() {
    //                 return Err(HandleWelcomeError::UnwantedGroup);
    //             }
    //         }
    //     }

    //     let required_members = self
    //         .key_retriever
    //         .get_required_group_members(&id)
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

    //     self.processor.join_group(staged).await?;

    //     Ok(())
    // }

    pub async fn send_report<Report: Serialize>(
        &self,
        report: Report,
    ) -> Result<MessageToServerTransport, SendDeviceMessageError> {
        let group_id = SvalinGroupId::DeviceGroup(self.me.spki_hash().clone()).to_group_id();
        let message = DeviceMessage::Report(report);
        let encoded = postcard::to_stdvec(&message)?;
        let to_server = self.processor.create_message(group_id, encoded).await?;

        Ok(to_server)
    }

    pub async fn create_device_group_if_missing(
        &self,
    ) -> Result<Option<MessageToServerTransport>, CreateDeviceGroupError<KeyRetriever::Error>> {
        let group_id = SvalinGroupId::DeviceGroup(self.me.spki_hash().clone());

        if self.processor.group_exists(group_id.to_group_id()).await? {
            return Ok(None);
        }

        let required_members = self
            .key_retriever
            .get_required_group_members(&group_id)
            .await
            .map_err(CreateDeviceGroupError::KeyRetrieverError)?
            .into_iter()
            .filter(|spki_hash| spki_hash != self.me.spki_hash())
            .collect::<Vec<_>>();

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

        let new_group = self
            .processor
            .create_group(members, group_id.to_group_id())
            .await?;

        Ok(Some(MessageToServerTransport::NewDeviceGroup {
            device_group: new_group,
        }))
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
pub enum CreateDeviceGroupError<KeyRetrieverError> {
    #[error("wrong certificate type: {0}, expected agent")]
    WrongCertificateType(CertificateType),
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
