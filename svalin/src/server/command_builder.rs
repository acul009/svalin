use std::sync::Arc;

use svalin_pki::mls::{self, provider::SvalinProvider};
use svalin_rpc::{
    commands::{forward::ForwardHandler, ping::PingHandler},
    rpc::{
        command::handler::HandlerCollection,
        server::{RpcServer, config_builder::RpcCommandBuilder},
    },
};

use crate::{
    permissions::default_permission_handler::DefaultPermissionHandler,
    server::{key_package_store::KeyPackageStore, session_store::SessionStore},
    shared::{
        commands::{
            agent_list::AgentListHandler,
            login::LoginHandler,
            public_server_status::{PublicStatus, PublicStatusHandler},
            upload_key_packages::UploadKeyPackagesHandler,
        },
        join_agent::add_agent::AddAgentHandler,
    },
    verifier::load_certificate_chain::LoadCertificateChainHandler,
};

use super::{agent_store::AgentStore, user_store::UserStore};

pub struct SvalinCommandBuilder {
    pub root_cert: svalin_pki::RootCertificate,
    pub server_cert: svalin_pki::Certificate,
    pub agent_store: Arc<AgentStore>,
    pub user_store: Arc<UserStore>,
    pub session_store: Arc<SessionStore>,
    pub key_package_store: Arc<KeyPackageStore>,
    pub mls_provider: Arc<SvalinProvider>,
}

const MLS_VERSION: mls::ProtocolVersion = mls::ProtocolVersion::Mls10;

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
            .add(LoadCertificateChainHandler::new(
                self.user_store.clone(),
                self.agent_store.clone(),
                self.session_store.clone(),
            ))
            .add(join_manager.create_request_handler())
            .add(join_manager.create_accept_handler())
            .add(ForwardHandler::new(server.clone()))
            .add(AddAgentHandler::new(
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
                MLS_VERSION,
                self.mls_provider.clone(),
            ));

        Ok(commands)
    }
}
