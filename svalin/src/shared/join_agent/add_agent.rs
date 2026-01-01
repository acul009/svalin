use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Certificate, CertificateChainBuilder, RootCertificate, UnverifiedCertificate,
    get_current_timestamp,
};
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::Session,
};
use tokio_util::sync::CancellationToken;

use crate::{
    permissions::Permission,
    server::{agent_store::AgentStore, user_store::UserStore},
};

#[derive(Serialize, Deserialize, Debug, thiserror::Error)]
enum AddAgentError {
    #[error("Error adding agent to store")]
    StoreError,
    #[error("Error verifying agent certificate")]
    VerificationError,
}

pub struct AddAgentHandler {
    agent_store: Arc<AgentStore>,
    user_store: Arc<UserStore>,
    root: RootCertificate,
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
    ) -> Result<Self> {
        Ok(Self {
            agent_store,
            user_store,
            root,
        })
    }
}

#[async_trait]
impl CommandHandler for AddAgentHandler {
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
        let cert_chain = CertificateChainBuilder::new(request);
        let cert_chain = match self.user_store.complete_certificate_chain(cert_chain).await {
            Err(err) => {
                session
                    .write_object::<Result<(), AddAgentError>>(&Err(
                        AddAgentError::VerificationError,
                    ))
                    .await?;

                return Err(err);
            }
            Ok(cert_chain) => cert_chain,
        };

        let agent_cert_chain = match cert_chain.verify(&self.root, get_current_timestamp()) {
            Err(err) => {
                session
                    .write_object::<Result<(), AddAgentError>>(&Err(
                        AddAgentError::VerificationError,
                    ))
                    .await?;

                return Err(anyhow!(err));
            }
            Ok(cert_chain) => cert_chain,
        };

        if let Err(err) = self
            .agent_store
            .add_agent(agent_cert_chain.take_leaf())
            .await
        {
            session
                .write_object::<Result<(), AddAgentError>>(&Err(AddAgentError::StoreError))
                .await?;

            return Err(err);
        } else {
            session
                .write_object::<Result<(), AddAgentError>>(&Ok(()))
                .await?;
        }

        Ok(())
    }
}

pub struct AddAgent<'a> {
    agent: &'a UnverifiedCertificate,
}

impl<'a> AddAgent<'a> {
    pub fn new(agent: &'a Certificate) -> Self {
        Self {
            agent: agent.as_unverified(),
        }
    }
}

impl<'a> CommandDispatcher for AddAgent<'a> {
    type Output = ();
    type Error = anyhow::Error;

    type Request = &'a UnverifiedCertificate;

    fn get_request(&self) -> &Self::Request {
        &self.agent
    }

    fn key() -> String {
        AddAgentHandler::key()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output> {
        session.read_object::<Result<(), AddAgentError>>().await??;

        Ok(())
    }
}
