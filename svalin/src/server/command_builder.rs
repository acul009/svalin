use std::sync::Arc;

use svalin_rpc::{
    commands::{forward::ForwardHandler, ping::PingHandler},
    rpc::{
        command::handler::HandlerCollection,
        server::{RpcServer, config_builder::RpcCommandBuilder},
    },
};

use crate::{
    permissions::server_permission_handler::ServerPermissionHandler,
    server::session_store::SessionStore,
    shared::{
        commands::{
            add_user::AddUserHandler,
            agent_list::AgentListHandler,
            login::LoginHandler,
            public_server_status::{PublicStatus, PublicStatusHandler},
        },
        join_agent::add_agent::AddAgentHandler,
    },
};

use super::{agent_store::AgentStore, user_store::UserStore};

pub struct SvalinCommandBuilder {
    pub root_cert: svalin_pki::Certificate,
    pub server_cert: svalin_pki::Certificate,
    pub agent_store: Arc<AgentStore>,
    pub user_store: Arc<UserStore>,
    pub session_store: Arc<SessionStore>,
}

impl RpcCommandBuilder for SvalinCommandBuilder {
    type PH = ServerPermissionHandler;

    async fn build(self, server: &Arc<RpcServer>) -> anyhow::Result<HandlerCollection<Self::PH>> {
        let permission_handler: ServerPermissionHandler =
            ServerPermissionHandler::new(self.root_cert.clone());

        let commands = HandlerCollection::new(permission_handler);

        let join_manager = crate::shared::join_agent::ServerJoinManager::new();

        commands
            .chain()
            .await
            .add(PingHandler)
            .add(PublicStatusHandler::new(PublicStatus::Ready))
            .add(LoginHandler::new(
                self.user_store.clone(),
                self.session_store.clone(),
                self.root_cert.clone(),
                self.server_cert.clone(),
            ))
            .add(AddUserHandler::new(self.user_store.clone()))
            .add(join_manager.create_request_handler())
            .add(join_manager.create_accept_handler())
            .add(ForwardHandler::new(server.clone()))
            .add(AddAgentHandler::new(self.agent_store.clone())?)
            .add(AgentListHandler::new(
                self.agent_store.clone(),
                server.clone(),
            ));

        Ok(commands)
    }
}
