use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Certificate, CertificateChainBuilder, RootCertificate, UnverifiedCertificate, VerifyChainError,
    get_current_timestamp, mls::server::AddDeviceGroupError,
};
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::{Session, SessionReadError, SessionWriteError},
};
use svalin_server_store::{AgentStore, CompleteCertChainError, MessageStoreError, UserStore};
use tokio_util::sync::CancellationToken;

use crate::permissions::Permission;

#[derive(Serialize, Deserialize, Debug, thiserror::Error)]
pub enum AddAgentError {
    #[error("Error adding agent to store")]
    StoreError,
    #[error("Error verifying agent certificate")]
    VerificationError,
}

#[derive(Debug, thiserror::Error)]
pub enum InternalAddAgentError {
    #[error("error building certificate chain: {0}")]
    CompleteCertChainError(#[from] CompleteCertChainError),
    #[error("error verifying loaded chain: {0}")]
    VerifyChainError(#[from] VerifyChainError),
    #[error("error saving agent to database: {0}")]
    AddAgentError(#[from] svalin_server_store::AddAgentError),
    #[error("error adding device group")]
    AddDeviceGroupError(AddDeviceGroupError<anyhow::Error>),
    #[error("error adding message to store: {0}")]
    MessageStoreError(#[from] MessageStoreError),
}

pub struct UploadAgentHandler {
    agent_store: Arc<AgentStore>,
    user_store: Arc<UserStore>,
    root: RootCertificate,
}

impl From<&PermissionPrecursor<UploadAgentHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<UploadAgentHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

impl UploadAgentHandler {
    pub fn new(
        agent_store: Arc<AgentStore>,
        user_store: Arc<UserStore>,
        root: RootCertificate,
    ) -> Result<Self> {
        Ok(Self {
            agent_store,
            user_store,
            root,
        })
    }
}

#[async_trait]
impl CommandHandler for UploadAgentHandler {
    type Request = UnverifiedCertificate;

    fn key() -> String {
        "add_agent".to_owned()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        _: CancellationToken,
    ) -> Result<()> {
        if let Err(err) = self.add_agent(request).await {
            session
                .write_object::<Result<(), AddAgentError>>(&Err(AddAgentError::StoreError))
                .await?;

            return Err(err.into());
        }

        session
            .write_object::<Result<(), AddAgentError>>(&Ok(()))
            .await?;

        Ok(())
    }
}

impl UploadAgentHandler {
    async fn add_agent(
        &self,
        certificate: UnverifiedCertificate,
    ) -> Result<(), InternalAddAgentError> {
        let cert_chain = CertificateChainBuilder::new(certificate);
        let cert_chain = self
            .user_store
            .complete_certificate_chain(cert_chain)
            .await?;
        let agent_cert_chain = cert_chain.verify(&self.root, get_current_timestamp())?;
        let agent_cert = agent_cert_chain.take_leaf();

        self.agent_store.add_agent(agent_cert).await?;

        Ok(())
    }
}

pub struct UploadAgent<'a>(&'a UnverifiedCertificate);

impl<'a> UploadAgent<'a> {
    pub fn new(device: &'a Certificate) -> Self {
        Self(device.as_unverified())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UploadAgentCommandError {
    #[error("error reading from session: {0}")]
    SessionReadError(#[from] SessionReadError),
    #[error("error writing to session: {0}")]
    SessionWriteError(#[from] SessionWriteError),
    #[error("error adding agent: {0}")]
    AddAgentError(#[from] AddAgentError),
}

impl<'a> CommandDispatcher for UploadAgent<'a> {
    type Output = ();
    type Error = UploadAgentCommandError;

    type Request = UnverifiedCertificate;

    fn get_request(&self) -> &Self::Request {
        &self.0
    }

    fn key() -> String {
        UploadAgentHandler::key()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        session.read_object::<Result<(), AddAgentError>>().await??;

        Ok(())
    }
}
