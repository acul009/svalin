use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Certificate, PermCredentials, get_current_timestamp, signed_object::SignedObject,
    verifier::exact::ExactVerififier,
};
use svalin_rpc::{
    commands::forward::ForwardConnection,
    rpc::{
        command::{
            dispatcher::CommandDispatcher,
            handler::{CommandHandler, PermissionPrecursor},
        },
        connection::direct_connection::DirectConnection,
        server::RpcServer,
        session::{Session, SessionReadError},
    },
};
use tokio::{select, sync::watch};
use tokio_util::sync::CancellationToken;

use crate::{
    client::{device::Device, tunnel_manager::TunnelManager},
    permissions::Permission,
    server::agent_store::{AgentStore, AgentUpdate},
    shared::join_agent::PublicAgentData,
};

#[derive(Clone, Debug)]
pub struct AgentListItem {
    pub public_data: PublicAgentData,
    pub is_online: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct AgentListItemTransport {
    pub public_data: SignedObject<PublicAgentData>,
    pub online_status: bool,
}

pub struct AgentListHandler {
    agent_store: Arc<AgentStore>,
    server: Arc<RpcServer>,
}

impl From<&PermissionPrecursor<AgentListHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<AgentListHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

impl AgentListHandler {
    pub fn new(agent_store: Arc<AgentStore>, server: Arc<RpcServer>) -> Self {
        Self {
            agent_store,
            server,
        }
    }
}

#[async_trait]
impl CommandHandler for AgentListHandler {
    type Request = ();

    fn key() -> String {
        "agent_list".into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _: Self::Request,
        cancel: CancellationToken,
    ) -> Result<()> {
        let mut receiver = self.server.subscribe_to_connection_status();
        let currently_online = self.server.get_current_connected_clients().await;

        let agents = self.agent_store.list_agents().await?;

        for agent in agents {
            let online_status = currently_online.contains(&agent.cert);
            let item = AgentListItemTransport {
                public_data: agent.pack_owned(),
                online_status,
            };

            session.write_object(&item).await?;
        }

        let mut agent_store_receiver = self.agent_store.subscribe();

        loop {
            select! {
                _ = cancel.cancelled() => return Ok(()),
                online_update = receiver.recv() => {
                    let online_update = online_update?;

                    let agent = self
                        .agent_store
                        .get_agent(&online_update.client.fingerprint()).await?;

                    if let Some(agent) = agent {
                        let item = AgentListItemTransport {
                            public_data: agent,
                            online_status: online_update.online,
                        };

                        // debug!("sending update to client: {}: {}", item.public_data.name, if item.online_status { "online"} else { "offline"});

                        session.write_object(&item).await?;
                    }
                },
                store_update = agent_store_receiver.recv() => {
                    let store_update = store_update?;

                    match store_update {
                        AgentUpdate::Add(public_data) => {
                            let online_status = self.server.is_client_connected(&public_data.cert).await;
                            let item = AgentListItemTransport {
                                public_data: public_data.pack().clone(),
                                online_status,
                            };

                            // debug!("sending update to client: {:?}", item);

                            session.write_object(&item).await?;
                        },
                    };

                }
            };
        }
    }
}

pub struct UpdateAgentList {
    pub base_connection: DirectConnection,
    pub credentials: PermCredentials,
    pub list: watch::Sender<BTreeMap<Certificate, Device>>,
    pub verifier: ExactVerififier,
    pub tunnel_manager: TunnelManager,
    pub cancel: CancellationToken,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateAgentListError {
    #[error("failed to receive AgentListItemTransport: {0}")]
    ReceiveItemError(SessionReadError),
    #[error("failed to verify AgentListItemTransport: {0}")]
    VerifyItemError(anyhow::Error),
}

#[async_trait]
impl CommandDispatcher for UpdateAgentList {
    type Output = ();
    type Error = UpdateAgentListError;

    type Request = ();

    fn key() -> String {
        AgentListHandler::key()
    }

    fn get_request(&self) -> Self::Request {}

    async fn dispatch(self, session: &mut Session, _: Self::Request) -> Result<(), Self::Error> {
        let cancel2 = self.cancel.clone();
        let result = self
            .cancel
            .run_until_cancelled(async move {
                loop {
                    let list_item_update: AgentListItemTransport = session
                        .read_object()
                        .await
                        .map_err(UpdateAgentListError::ReceiveItemError)?;

                    let public_data = list_item_update
                        .public_data
                        .verify(&self.verifier, get_current_timestamp())
                        .await
                        .map_err(UpdateAgentListError::VerifyItemError)?;

                    let item = AgentListItem {
                        is_online: list_item_update.online_status,
                        public_data: public_data.unpack(),
                    };

                    self.list.send_modify(|list| {
                        if let Some(device) = list.get(&item.public_data.cert) {
                            device.update(item);
                        } else {
                            let device_connection = ForwardConnection::new(
                                self.base_connection.clone(),
                                self.credentials.clone(),
                                item.public_data.cert.clone(),
                            );

                            let cert = item.public_data.cert.clone();
                            let device = Device::new(
                                device_connection,
                                item,
                                self.tunnel_manager.clone(),
                                cancel2.clone(),
                            );

                            list.insert(cert, device);
                        }
                    });
                }
            })
            .await;

        match result {
            Some(result) => result,
            None => Ok(()),
        }
    }
}
