use std::sync::Arc;

use svalin_rpc::{
    commands::{forward::ForwardHandler, ping::PingHandler},
    rpc::{
        command::handler::HandlerCollection,
        server::{RpcServer, config_builder::RpcCommandBuilder},
    },
};

use crate::{
    permissions::default_permission_handler::DefaultPermissionHandler,
    server::{
        MlsServer, chain_loader::ChainLoader, key_package_store::KeyPackageStore,
        message_store::MessageStore, session_store::SessionStore,
    },
    shared::{
        commands::{
            agent_list::AgentListHandler,
            get_key_packages::GetKeyPackagesHandler,
            load_certificate_chain::LoadCertificateChainHandler,
            login::LoginHandler,
            public_server_status::{PublicStatus, PublicStatusHandler},
            upload_key_packages::UploadKeyPackagesHandler,
        },
        join_agent::upload_agent::UploadAgentHandler,
    },
};

use super::{agent_store::AgentStore, user_store::UserStore};

pub struct SvalinCommandBuilder {
    pub root_cert: svalin_pki::RootCertificate,
    pub server_cert: svalin_pki::Certificate,
    pub agent_store: Arc<AgentStore>,
    pub user_store: Arc<UserStore>,
    pub session_store: Arc<SessionStore>,
    pub key_package_store: Arc<KeyPackageStore>,
    pub message_store: Arc<MessageStore>,
    pub mls: Arc<MlsServer>,
}

impl RpcCommandBuilder for SvalinCommandBuilder {
    type PH = DefaultPermissionHandler;

    async fn build(self, server: &Arc<RpcServer>) -> anyhow::Result<HandlerCollection<Self::PH>> {
        let permission_handler: DefaultPermissionHandler =
            DefaultPermissionHandler::new(self.root_cert.clone());

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
            .add(LoadCertificateChainHandler::new(ChainLoader::new(
                self.user_store.clone(),
                self.agent_store.clone(),
                self.session_store.clone(),
            )))
            .add(join_manager.create_request_handler())
            .add(join_manager.create_accept_handler())
            .add(ForwardHandler::new(server.clone()))
            .add(UploadAgentHandler::new(
                self.agent_store.clone(),
                self.user_store.clone(),
                self.root_cert.clone(),
            )?)
            .add(AgentListHandler::new(
                self.agent_store.clone(),
                server.clone(),
            ))
            .add(UploadKeyPackagesHandler::new(
                self.key_package_store.clone(),
                self.mls,
            ))
            .add(GetKeyPackagesHandler::new(self.key_package_store.clone()));

        Ok(commands)
    }
}
