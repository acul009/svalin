use std::sync::Arc;

use svalin_pki::mls::transport_types::MessageToServerTransport;
use svalin_rpc::{
    commands::{forward::ForwardHandler, ping::PingHandler},
    rpc::{
        command::handler::HandlerCollection,
        server::{RpcServer, config_builder::RpcCommandBuilder},
    },
};
use svalin_server_store::ServerStore;
use tokio::sync::mpsc;

use crate::{
    permissions::default_permission_handler::DefaultPermissionHandler,
    server::{MlsServer, chain_loader::ChainLoader},
    shared::{
        commands::{
            agent_list::AgentListHandler,
            get_key_packages::GetKeyPackagesHandler,
            load_certificate_chain::LoadCertificateChainHandler,
            login::LoginHandler,
            mls::upload_mls::UploadMlsHandler,
            public_server_status::{PublicStatus, PublicStatusHandler},
            update_user_mls::UpdateUserMlsHandler,
            upload_key_packages::UploadKeyPackagesHandler,
        },
        join_agent::upload_agent::UploadAgentHandler,
    },
};

pub struct SvalinCommandBuilder {
    pub root_cert: svalin_pki::RootCertificate,
    pub server_cert: svalin_pki::Certificate,
    pub store: ServerStore,
    pub mls: Arc<MlsServer>,
    pub to_mls: mpsc::Sender<MessageToServerTransport>,
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
                self.store.users.clone(),
                self.store.sessions.clone(),
                self.root_cert.clone(),
                self.server_cert.clone(),
            ))
            .add(LoadCertificateChainHandler::new(ChainLoader::new(
                self.store.users.clone(),
                self.store.agents.clone(),
                self.store.sessions.clone(),
            )))
            .add(join_manager.create_request_handler())
            .add(join_manager.create_accept_handler())
            .add(ForwardHandler::new(server.clone()))
            .add(UploadAgentHandler::new(
                self.store.agents.clone(),
                self.store.users.clone(),
                self.root_cert.clone(),
            )?)
            .add(AgentListHandler::new(
                self.store.agents.clone(),
                server.clone(),
            ))
            .add(UploadKeyPackagesHandler::new(
                self.store.key_packages.clone(),
                self.mls.clone(),
            ))
            .add(GetKeyPackagesHandler::new(self.store.key_packages.clone()))
            .add(UploadMlsHandler(self.to_mls))
            .add(UpdateUserMlsHandler::new(
                self.store.users.clone(),
                self.store.messages.clone(),
                self.store.key_packages.clone(),
                self.mls.clone(),
            ));

        Ok(commands)
    }
}
