use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_macros::rpc_dispatch;
use svalin_pki::{signed_object::SignedObject, Certificate, PermCredentials};
use svalin_rpc::{
    commands::forward::ForwardConnection,
    rpc::{
        command::handler::CommandHandler,
        connection::DirectConnection,
        server::RpcServer,
        session::{Session},
    },
};
use tokio::{select, sync::RwLock};
use tracing::debug;

use crate::{
    client::device::Device,
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

    async fn handle(&self, session: &mut Session) -> Result<()> {
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

                    let agent = self
                        .agent_store
                        .get_agent(online_update.client.public_key())?;

                    if let Some(agent) = agent {
                        let item = AgentListItemTransport {
                            public_data: agent,
                            online_status: online_update.online,
                        };

                        debug!("sending update to client: {}: {}", item.public_data.name, if item.online_status { "online"} else { "offline"});

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
pub async fn update_agent_list(
    session: &mut Session,
    base_connection: DirectConnection,
    credentials: PermCredentials,
    list: Arc<RwLock<BTreeMap<Certificate, Device>>>,
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

        {
            if let Some(device) = list.read().await.get(&item.public_data.cert) {
                // Either we update the device...

                device.update(item).await;
                continue;
            }
        }

        {
            // ...or we create it

            let device_connection = ForwardConnection::new(
                base_connection.clone(),
                credentials.clone(),
                item.public_data.cert.clone(),
            );

            let cert = item.public_data.cert.clone();

            let device = Device::new(device_connection, item);

            list.write().await.insert(cert, device);
        }
    }
}
