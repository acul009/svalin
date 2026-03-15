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
