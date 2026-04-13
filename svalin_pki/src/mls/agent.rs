use std::collections::HashSet;

use openmls::{error::LibraryError, prelude::Welcome};
use openmls_sqlx_storage::SqliteStorageProvider;
use serde::Serialize;
use tokio::task::JoinError;

use crate::{
    Certificate, CertificateType, Credential, SpkiHash,
    mls::{
        SvalinGroupId,
        group_id::ParseGroupIdError,
        key_package::KeyPackage,
        processor::{
            CreateGroupMessageError, CreateKeyPackageError, JoinGroupError, MlsProcessorHandle,
        },
        provider::{PostcardCodec, SvalinProvider},
        transport_types::{DeviceMessage, MessageToMember, MessageToServerTransport},
    },
};

pub struct MlsAgent<KeyRetriever, Verifier> {
    processor: MlsProcessorHandle,
    key_retriever: KeyRetriever,
    verifier: Verifier,
    me: Certificate,
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
            me,
            processor,
            key_retriever,
            verifier,
        })
    }

    pub async fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        self.processor.create_key_package().await
    }

    pub async fn handle_message(
        &self,
        message: MessageToMember,
    ) -> Result<(), HandleMessageError<KeyRetriever::Error>> {
        match message {
            MessageToMember::Welcome(welcome) => self
                .handle_welcome(welcome)
                .await
                .map_err(HandleMessageError::Welcome),
            MessageToMember::GroupMessage(private_message_in) => todo!(),
        }
    }

    async fn handle_welcome(
        &self,
        welcome: Welcome,
    ) -> Result<(), HandleWelcomeError<KeyRetriever::Error>> {
        let staged = self.processor.stage_join(welcome).await?;
        let id = SvalinGroupId::from_group_id(staged.group_context().group_id())?;

        match &id {
            SvalinGroupId::DeviceGroup(device) => {
                if device != self.me.spki_hash() {
                    return Err(HandleWelcomeError::UnwantedGroup);
                }
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

        Ok(())
    }

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

    // pub async fn join_my_device_group(
    //     &self,
    //     group_info: DeviceGroupCreationInfo,
    // ) -> Result<(), JoinDeviceGroupError> {
    //     let provider = self.provider.clone();
    //     let me = self.svalin_credential.get_certificate().spki_hash().clone();
    //     let my_parent = self.svalin_credential.get_certificate().issuer().clone();

    //     let welcome = group_info.welcome()?;

    //     let ratchet_tree = group_info.ratchet_tree()?;

    //     let join_config = MlsGroupJoinConfig::builder()
    //         .max_past_epochs(0)
    //         .use_ratchet_tree_extension(false)
    //         .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
    //         .sender_ratchet_configuration(SenderRatchetConfiguration::new(0, 0))
    //         .build();

    //     let welcome = StagedWelcome::new_from_welcome(
    //         provider.as_ref(),
    //         &join_config,
    //         welcome,
    //         Some(ratchet_tree),
    //     )?;

    //     if welcome.group_context().group_id().as_slice() != me.as_slice() {
    //         return Err(JoinDeviceGroupError::WrongGroupId);
    //     }

    //     let creator: UnverifiedCertificate =
    //         welcome.welcome_sender()?.credential().deserialized()?;

    //     // Ensure there are only sessions and myself in the group
    //     welcome
    //         .members()
    //         .map(|member| -> Result<(), JoinDeviceGroupError> {
    //             let certificate: UnverifiedCertificate = member.credential.deserialized()?;
    //             if certificate.spki_hash() == &me {
    //                 return Ok(());
    //             }

    //             if certificate.certificate_type() != CertificateType::UserDevice {
    //                 return Err(JoinDeviceGroupError::WrongMemberType);
    //             }

    //             Ok(())
    //         })
    //         .collect::<Result<(), JoinDeviceGroupError>>()?;

    //     // TODO: check that members contains root
    //     if creator.issuer() != &my_parent {
    //         return Err(JoinDeviceGroupError::WrongGroupCreator);
    //     }

    //     let _group = welcome.into_group(provider.as_ref())?;

    //     Ok(())
    // }
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
