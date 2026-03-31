use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Certificate, CertificateChainBuilder, RootCertificate, SpkiHash, UnverifiedCertificate,
    VerifyChainError, get_current_timestamp,
    mls::{server::AddDeviceGroupError, transport_types::NewGroupTransport},
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
        MlsServer,
        agent_store::{self, AgentStore},
        message_store::{MessageStore, MessageStoreError},
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
    #[error("error adding device group")]
    AddDeviceGroupError(AddDeviceGroupError<anyhow::Error>),
    #[error("error adding message to store: {0}")]
    MessageStoreError(#[from] MessageStoreError),
}

pub struct UploadAgentHandler {
    agent_store: Arc<AgentStore>,
    user_store: Arc<UserStore>,
    message_store: Arc<MessageStore>,
    root: RootCertificate,
    mls: Arc<MlsServer>,
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
        message_store: Arc<MessageStore>,
        root: RootCertificate,
        mls: Arc<MlsServer>,
    ) -> Result<Self> {
        Ok(Self {
            agent_store,
            user_store,
            message_store,
            root,
            mls,
        })
    }
}

#[async_trait]
impl CommandHandler for UploadAgentHandler {
    type Request = UploadAgentData;

    fn key() -> String {
        "add_agent".to_owned()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        _: CancellationToken,
    ) -> Result<()> {
        let svalin_rpc::rpc::peer::Peer::Certificate(sender) = session.peer() else {
            panic!("unexpected peer type: {:?}", session.peer())
        };

        if let Err(err) = self.add_agent(request, sender.spki_hash()).await {
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
        data: UploadAgentData,
        sender: &SpkiHash,
    ) -> Result<(), InternalAddAgentError> {
        let cert_chain = CertificateChainBuilder::new(data.certificate);
        let cert_chain = self
            .user_store
            .complete_certificate_chain(cert_chain)
            .await?;
        let agent_cert_chain = cert_chain.verify(&self.root, get_current_timestamp())?;
        let agent_cert = agent_cert_chain.take_leaf();
        let spki_hash = agent_cert.spki_hash().clone();

        self.agent_store.add_agent(agent_cert).await?;

        let mut to_send = self
            .mls
            .add_device_group(data.device_group, &spki_hash)
            .await
            .map_err(InternalAddAgentError::AddDeviceGroupError)?;

        // Do not deliver the welcome to the sender or the agent themselves
        to_send.receivers = to_send
            .receivers
            .into_iter()
            .filter(|receiver| receiver != sender && receiver != &spki_hash)
            .collect();

        self.message_store.add_message(to_send).await?;

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct UploadAgentData {
    certificate: UnverifiedCertificate,
    device_group: NewGroupTransport,
}

pub struct UploadAgent(UploadAgentData);

impl UploadAgent {
    pub fn new(device: Certificate, device_group: NewGroupTransport) -> Self {
        Self(UploadAgentData {
            certificate: device.to_unverified(),
            device_group,
        })
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

impl CommandDispatcher for UploadAgent {
    type Output = ();
    type Error = UploadAgentCommandError;

    type Request = UploadAgentData;

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
