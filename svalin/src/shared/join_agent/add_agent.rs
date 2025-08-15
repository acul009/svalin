use std::{error::Error, fmt::Display, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::SignedObject;
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::Session,
};
use tokio_util::sync::CancellationToken;

use crate::{permissions::Permission, server::agent_store::AgentStore};

use super::PublicAgentData;

#[derive(Serialize, Deserialize, Debug)]
enum AddAgentError {
    StoreError,
}

impl Error for AddAgentError {}

impl Display for AddAgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddAgentError::StoreError => write!(f, "Error adding agent to store"),
        }
    }
}

pub struct AddAgentHandler {
    store: Arc<AgentStore>,
}

impl From<&PermissionPrecursor<AddAgentHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<AddAgentHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

impl AddAgentHandler {
    pub fn new(store: Arc<AgentStore>) -> Result<Self> {
        Ok(Self { store })
    }
}

#[async_trait]
impl CommandHandler for AddAgentHandler {
    type Request = SignedObject<PublicAgentData>;

    fn key() -> String {
        "add_agent".to_owned()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        _: CancellationToken,
    ) -> Result<()> {
        let agent = request;

        if let Err(err) = self.store.add_agent(agent).await {
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
    pub agent: &'a SignedObject<PublicAgentData>,
}

impl<'a> CommandDispatcher for AddAgent<'a> {
    type Output = ();
    type Error = anyhow::Error;

    type Request = &'a SignedObject<PublicAgentData>;

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
