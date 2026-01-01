use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Certificate, Credential, KnownCertificateVerifier, SpkiHash, UnverifiedCertificate,
    VerifyError, get_current_timestamp,
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
    client::{Client, device::Device},
    permissions::Permission,
    server::agent_store::{AgentStore, AgentUpdate},
    verifier::remote_agent_verifier::RemoteAgentVerifier,
};

#[derive(Clone, Debug)]
pub struct AgentListItem {
    pub certificate: Certificate,
    pub online_status: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct AgentListItemTransport {
    pub certificate: UnverifiedCertificate,
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
            let online_status = currently_online.contains(&agent.spki_hash());
            let item = AgentListItemTransport {
                certificate: agent,
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

                    let agent_certificate = self
                        .agent_store
                        .get_agent(&online_update.client.spki_hash()).await?;

                    if let Some(agent) = agent_certificate {
                        let item = AgentListItemTransport {
                            certificate: agent,
                            online_status: online_update.online,
                        };

                        // debug!("sending update to client: {}: {}", item.public_data.name, if item.online_status { "online"} else { "offline"});

                        session.write_object(&item).await?;
                    }
                },
                store_update = agent_store_receiver.recv() => {
                    let store_update = store_update?;

                    match store_update {
                        AgentUpdate::Add(agent_certificate) => {
                            let online_status = self.server.is_client_connected(&agent_certificate).await;
                            let item = AgentListItemTransport {
                                certificate: agent_certificate.to_unverified(),
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
    pub client: Arc<Client>,
    pub credentials: Credential,
    pub list: watch::Sender<BTreeMap<SpkiHash, Device>>,
    pub verifier: RemoteAgentVerifier,
    pub cancel: CancellationToken,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateAgentListError {
    #[error("failed to receive AgentListItemTransport: {0}")]
    ReceiveItemError(#[from] SessionReadError),
    #[error("failed to verify AgentListItemTransport: {0}")]
    VerifyItemError(#[from] VerifyError),
}

impl CommandDispatcher for UpdateAgentList {
    type Output = ();
    type Error = UpdateAgentListError;

    type Request = ();

    fn key() -> String {
        AgentListHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(self, session: &mut Session) -> Result<(), Self::Error> {
        let result = self
            .cancel
            .run_until_cancelled(async move {
                loop {
                    let list_item_update: AgentListItemTransport = session
                        .read_object()
                        .await
                        .map_err(UpdateAgentListError::ReceiveItemError)?;

                    let certificate = self
                        .verifier
                        .verify_known_certificate(
                            &list_item_update.certificate,
                            get_current_timestamp(),
                        )
                        .await?;

                    let item = AgentListItem {
                        online_status: list_item_update.online_status,
                        certificate,
                    };

                    self.list.send_modify(|list| {
                        if let Some(device) = list.get(&item.certificate.spki_hash()) {
                            device.update(item);
                        } else {
                            let spki_hash = item.certificate.spki_hash().clone();
                            let device_connection = ForwardConnection::new(
                                self.base_connection.clone(),
                                self.credentials.clone(),
                                item.certificate.clone(),
                            );

                            let device = Device::new(device_connection, item, self.client.clone());

                            list.insert(spki_hash, device);
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
