use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_macros::rpc_dispatch;
use svalin_pki::{signed_object::SignedObject, Certificate};
use svalin_rpc::rpc::{
    command::CommandHandler,
    server::RpcServer,
    session::{Session, SessionOpen},
};
use tokio::{select, sync::RwLock};
use tracing::debug;

use crate::{
    server::agent_store::{AgentStore, AgentUpdate},
    shared::join_agent::PublicAgentData,
};

#[derive(Clone, Debug)]
pub struct AgentListItem {
    pub public_data: PublicAgentData,
    pub online_status: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct AgentListItemTransport {
    pub public_data: SignedObject<PublicAgentData>,
    pub online_status: bool,
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
            let item = AgentListItemTransport {
                public_data: agent,
                online_status,
            };

            session.write_object(&item).await?;
        }

        let mut agent_store_receiver = self.agent_store.subscribe();

        loop {
            select! {
                online_update = receiver.recv() => {
                    let online_update = online_update?;

                    debug!("Online update from server: {:?}", online_update);

                    let agent = self
                        .agent_store
                        .get_agent(online_update.client.public_key())?;

                    debug!("retrieved agent from store: {:?}", agent);

                    if let Some(agent) = agent {
                        let item = AgentListItemTransport {
                            public_data: agent,
                            online_status: online_update.online,
                        };

                        debug!("sending update to client: {:?}", item);

                        session.write_object(&item).await?;
                    }
                },
                store_update = agent_store_receiver.recv() => {
                    let store_update = store_update?;

                    match store_update {
                        AgentUpdate::Add(public_data) => {
                            let online_status = self.server.is_client_connected(&public_data.cert).await;
                            let item = AgentListItemTransport {
                                public_data,
                                online_status,
                            };

                            debug!("sending update to client: {:?}", item);

                            session.write_object(&item).await?;
                        },
                    };

                }
            };
        }
    }
}

#[rpc_dispatch(agent_list_key())]
pub async fn agent_list(
    session: &mut Session<SessionOpen>,
    list: Arc<RwLock<BTreeMap<Certificate, AgentListItem>>>,
) -> Result<()> {
    loop {
        let list_item_update: AgentListItemTransport = session
            .read_object()
            .await
            .context("failed to receive ItemTransport")?;

        let item = AgentListItem {
            online_status: list_item_update.online_status,
            public_data: list_item_update.public_data.unpack(),
        };

        list.write()
            .await
            .insert(item.public_data.cert.clone(), item);
    }
}
