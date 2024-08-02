use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::signed_object::SignedObject;
use svalin_rpc::rpc::{
    command::CommandHandler,
    server::RpcServer,
    session::{Session, SessionOpen},
};
use tokio::sync::broadcast;

use crate::{agent, server::agent_store::AgentStore, shared::join_agent::PublicAgentData};

#[derive(Serialize, Deserialize, Debug)]
struct AgentListItem {
    public_data: SignedObject<PublicAgentData>,
    online_status: bool,
}

pub struct AgentListHandler {
    agent_store: Arc<AgentStore>,
    server: RpcServer,
}

impl AgentListHandler {
    pub fn new(agent_store: Arc<AgentStore>, server: RpcServer) -> Self {
        Self {
            agent_store,
            server,
        }
    }
}

fn agent_list_key() -> String {
    "agent_list".into()
}

#[async_trait]
impl CommandHandler for AgentListHandler {
    fn key(&self) -> String {
        agent_list_key()
    }

    async fn handle(&self, session: &mut Session<SessionOpen>) -> Result<()> {
        let mut receiver = self.server.subscribe_to_connection_status();
        let currently_online = self.server.get_current_connected_clients().await;

        let agents = self.agent_store.list_agents()?;

        for agent in agents {
            let online_status = currently_online.contains(&agent.cert);
            let item = AgentListItem {
                public_data: agent,
                online_status,
            };

            session.write_object(&item).await?;
        }

        loop {
            let online_update = receiver.recv().await?;

            let agent = self
                .agent_store
                .get_agent(online_update.client.public_key())?;

            if let Some(agent) = agent {
                let item = AgentListItem {
                    public_data: agent,
                    online_status: online_update.online,
                };

                session.write_object(&item).await?;
            }
        }
    }
}
