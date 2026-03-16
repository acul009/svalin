use std::marker::PhantomData;

use openmls::{
    framing::errors::MlsMessageError,
    group::{CreateMessageError, GroupId, MlsGroup},
    storage::StorageProvider,
};
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::task::JoinError;

use crate::{
    Certificate, CertificateType, Credential, SpkiHash,
    mls::{
        key_package::KeyPackage,
        processor::{
            CreateGroupMessageError, CreateKeyPackageError, GroupMessage, MlsProcessorHandle,
        },
        provider::{PostcardCodec, SvalinProvider},
    },
};

pub struct MlsAgent {
    mls: MlsProcessorHandle,
    id: SpkiHash,
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

impl MlsAgent {
    pub async fn new(
        credential: Credential,
        storage_provider: SqliteStorageProvider<PostcardCodec>,
    ) -> Result<Self, MlsAgentCreateError> {
        if credential.get_certificate().certificate_type() != CertificateType::Agent {
            return Err(MlsAgentCreateError::NotAnAgent(
                credential.get_certificate().clone(),
            ));
        }

        let id = credential.get_certificate().spki_hash().clone();

        let mls = MlsProcessorHandle::new_processor(credential, storage_provider);

        Ok(Self { id, mls })
    }

    pub async fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        self.mls.create_key_package().await
    }

    pub async fn join_my_device_group(
        &self,
        group_info: DeviceGroupCreationInfo,
    ) -> Result<(), JoinDeviceGroupError> {
        let provider = self.provider.clone();
        let me = self.svalin_credential.get_certificate().spki_hash().clone();
        let my_parent = self.svalin_credential.get_certificate().issuer().clone();

        let welcome = group_info.welcome()?;

        let ratchet_tree = group_info.ratchet_tree()?;

        let join_config = MlsGroupJoinConfig::builder()
            .max_past_epochs(0)
            .use_ratchet_tree_extension(false)
            .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .sender_ratchet_configuration(SenderRatchetConfiguration::new(0, 0))
            .build();

        let welcome = StagedWelcome::new_from_welcome(
            provider.as_ref(),
            &join_config,
            welcome,
            Some(ratchet_tree),
        )?;

        if welcome.group_context().group_id().as_slice() != me.as_slice() {
            return Err(JoinDeviceGroupError::WrongGroupId);
        }

        let creator: UnverifiedCertificate =
            welcome.welcome_sender()?.credential().deserialized()?;

        // Ensure there are only sessions and myself in the group
        welcome
            .members()
            .map(|member| -> Result<(), JoinDeviceGroupError> {
                let certificate: UnverifiedCertificate = member.credential.deserialized()?;
                if certificate.spki_hash() == &me {
                    return Ok(());
                }

                if certificate.certificate_type() != CertificateType::UserDevice {
                    return Err(JoinDeviceGroupError::WrongMemberType);
                }

                Ok(())
            })
            .collect::<Result<(), JoinDeviceGroupError>>()?;

        // TODO: check that members contains root
        if creator.issuer() != &my_parent {
            return Err(JoinDeviceGroupError::WrongGroupCreator);
        }

        let _group = welcome.into_group(provider.as_ref())?;

        Ok(())
    }

    pub async fn create_new_report<Report: Serialize>(
        &self,
        report: &Report,
    ) -> Result<EncodedReport<Report>, CreateSystemreportError> {
        let report = postcard::to_stdvec(report)?;

        let message = self.mls.create_message(self.id.to_vec(), report).await?;

        Ok(EncodedReport {
            message,
            report: PhantomData,
        })
    }
}

#[derive(Serialize, serde::Deserialize)]
pub struct EncodedReport<Report> {
    message: GroupMessage,
    report: PhantomData<Report>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateSystemreportError {
    #[error("postcard error: {0}")]
    PostcardError(#[from] postcard::Error),
    #[error("create message error: {0}")]
    CreateMessageError(#[from] CreateGroupMessageError),
}
