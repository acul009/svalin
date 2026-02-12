use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    CertificateChainBuilder, RootCertificate, VerifyChainError, get_current_timestamp,
    mls::{
        client::DeviceGroupCreationInfo,
        delivery_service::{self, DeliveryService, NewPublicDeviceGroupError},
    },
};
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::{Session, SessionReadError, SessionWriteError},
};
use tokio_util::sync::CancellationToken;

use crate::{
    permissions::Permission,
    server::{
        agent_store::{self, AgentStore},
        user_store::{CompleteCertChainError, UserStore},
    },
};

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
    AddAgentError(#[from] agent_store::AddAgentError),
    #[error("error creating public device group")]
    NewDeviceGroupError(#[from] NewPublicDeviceGroupError),
}

pub struct AddAgentHandler {
    agent_store: Arc<AgentStore>,
    user_store: Arc<UserStore>,
    root: RootCertificate,
    delivery_service: Arc<DeliveryService>,
}

impl From<&PermissionPrecursor<AddAgentHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<AddAgentHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

impl AddAgentHandler {
    pub fn new(
        agent_store: Arc<AgentStore>,
        user_store: Arc<UserStore>,
        root: RootCertificate,
        delivery_service: Arc<DeliveryService>,
    ) -> Result<Self> {
        Ok(Self {
            agent_store,
            user_store,
            root,
            delivery_service,
        })
    }
}

#[async_trait]
impl CommandHandler for AddAgentHandler {
    type Request = DeviceGroupCreationInfo;

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

impl AddAgentHandler {
    async fn add_agent(
        &self,
        group_info: DeviceGroupCreationInfo,
    ) -> Result<(), InternalAddAgentError> {
        let cert_chain = CertificateChainBuilder::new(group_info.certificate().clone());
        let cert_chain = self
            .user_store
            .complete_certificate_chain(cert_chain)
            .await?;
        let agent_cert_chain = cert_chain.verify(&self.root, get_current_timestamp())?;

        self.agent_store
            .add_agent(agent_cert_chain.take_leaf())
            .await?;

        self.delivery_service.new_device_group(group_info).await?;

        Ok(())
    }
}

pub struct UploadAgent<'a> {
    group_info: &'a DeviceGroupCreationInfo,
}

impl<'a> UploadAgent<'a> {
    pub fn new(group_info: &'a DeviceGroupCreationInfo) -> Self {
        Self { group_info }
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

    type Request = &'a DeviceGroupCreationInfo;

    fn get_request(&self) -> &Self::Request {
        &self.group_info
    }

    fn key() -> String {
        AddAgentHandler::key()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        session.read_object::<Result<(), AddAgentError>>().await??;

        Ok(())
    }
}
