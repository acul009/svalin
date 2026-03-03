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
    Certificate, CertificateType, Credential,
    mls::{
        client::MlsClient,
        provider::{PostcardCodec, SvalinProvider},
    },
};

pub struct MlsAgent<Report> {
    mls: MlsClient,
    group: tokio::sync::Mutex<MlsGroup>,
    report: PhantomData<Report>,
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

impl<Report> MlsAgent<Report>
where
    Report: Serialize + DeserializeOwned,
{
    pub async fn new(
        credential: Credential,
        storage_provider: SqliteStorageProvider<PostcardCodec>,
    ) -> Result<Self, MlsAgentCreateError> {
        if credential.get_certificate().certificate_type() != CertificateType::Agent {
            return Err(MlsAgentCreateError::NotAnAgent(
                credential.get_certificate().clone(),
            ));
        }

        let group_id = GroupId::from_slice(credential.get_certificate().spki_hash().as_slice());

        let (group, storage_provider) = tokio::task::spawn_blocking(move || {
            let group_id = group_id;
            (
                MlsGroup::load(&storage_provider, &group_id)
                    .map_err(MlsAgentCreateError::StorageError)
                    .map(|opt| opt.ok_or_else(|| MlsAgentCreateError::MissingMyGroup))
                    .flatten(),
                storage_provider,
            )
        })
        .await?;

        let group = group?;

        let mls = MlsClient::new(credential, storage_provider);

        Ok(Self {
            mls,
            group: tokio::sync::Mutex::new(group),
            report: PhantomData,
        })
    }

    pub async fn create_new_report(
        &self,
        report: &Report,
    ) -> Result<EncodedReport<Report>, CreateSystemreportError> {
        let report = postcard::to_stdvec(report)?;

        let mut guard = self.group.lock().await;

        let message = guard.create_message(self.mls.provider(), self.mls.signer(), &report)?;

        let bytes = message.to_bytes()?;

        Ok(EncodedReport {
            bytes,
            report: PhantomData,
        })
    }
}

#[derive(Serialize, serde::Deserialize)]
pub struct EncodedReport<Report> {
    bytes: Vec<u8>,
    report: PhantomData<Report>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateSystemreportError {
    #[error("postcard error: {0}")]
    PostcardError(#[from] postcard::Error),
    #[error("create message error: {0}")]
    CreateMessageError(#[from] CreateMessageError),
    #[error("mls message error")]
    MlsError(#[from] MlsMessageError),
}
