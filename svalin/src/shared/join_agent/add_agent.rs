use std::{error::Error, fmt::Display, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_macros::rpc_dispatch;
use svalin_pki::signed_object::SignedObject;
use svalin_rpc::rpc::{
    command::CommandHandler,
    session::{Session, SessionOpen},
};

use crate::server::agent_store::AgentStore;

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

impl AddAgentHandler {
    pub fn new(store: Arc<AgentStore>) -> Result<Self> {
        Ok(Self { store })
    }
}

fn add_agent_key() -> String {
    "add_agent".to_owned()
}

#[async_trait]
impl CommandHandler for AddAgentHandler {
    fn key(&self) -> String {
        add_agent_key()
    }

    async fn handle(&self, session: &mut Session<SessionOpen>) -> Result<()> {
        let agent = SignedObject::<PublicAgentData>::from_bytes(session.read_object().await?)?;

        if let Err(err) = self.store.add_agent(agent) {
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

#[rpc_dispatch(add_agent_key())]
pub async fn add_agent(
    session: &mut Session<SessionOpen>,
    agent: &SignedObject<PublicAgentData>,
) -> Result<()> {
    session.write_object(&agent.to_bytes()).await?;

    session.read_object::<Result<(), AddAgentError>>().await??;

    Ok(())
}
