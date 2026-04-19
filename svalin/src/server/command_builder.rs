use std::sync::Arc;

use svalin_rpc::{
    commands::{forward::ForwardHandler, ping::PingHandler},
    rpc::{
        command::handler::HandlerCollection,
        server::{RpcServer, config_builder::RpcCommandBuilder},
    },
};
use svalin_server_store::ServerStore;

use crate::{
    message_streaming::{
        server::MlsMessageHandler,
        with_agent::{self},
        with_client,
    },
    permissions::default_permission_handler::DefaultPermissionHandler,
    server::{MlsServer, chain_loader::ChainLoader},
    shared::{
        commands::{
            get_key_packages::GetKeyPackagesHandler,
            load_certificate_chain::LoadCertificateChainHandler,
            login::LoginHandler,
            public_server_status::{PublicStatus, PublicStatusHandler},
            update_user_mls::UpdateUserMlsHandler,
        },
        join_agent::upload_agent::UploadAgentHandler,
    },
    verifier::local_verifier::LocalVerifier,
};

pub struct SvalinCommandBuilder {
    pub root_cert: svalin_pki::RootCertificate,
    pub server_cert: svalin_pki::Certificate,
    pub store: ServerStore,
    pub mls: Arc<MlsServer>,
    pub verifier: LocalVerifier,
}

impl RpcCommandBuilder for SvalinCommandBuilder {
    type PH = DefaultPermissionHandler;

    async fn build(self, server: &Arc<RpcServer>) -> anyhow::Result<HandlerCollection<Self::PH>> {
        let permission_handler: DefaultPermissionHandler =
            DefaultPermissionHandler::new(self.root_cert.clone());

        let commands = HandlerCollection::new(permission_handler);

        let join_manager = crate::shared::join_agent::ServerJoinManager::new();

        let agent_sender = with_agent::MessageSender::new();
        let client_sender =
            with_client::MessageSender::new(server.clone(), self.store.messages.clone());

        let mls_handler = Arc::new(MlsMessageHandler {
            key_package_store: self.store.key_packages.clone(),
            message_store: self.store.messages.clone(),
            mls_server: self.mls.clone(),
            verifier: self.verifier.clone(),
        });

        let agent_message_handler = with_agent::MessageHandler {
            mls_handler: mls_handler.clone(),
        };
        let client_message_handler = with_client::MessageHandler { mls_handler };

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
            // .add(AgentListHandler::new(
            //     self.store.agents.clone(),
            //     server.clone(),
            // ))
            // .add(UploadKeyPackagesHandler {
            //     key_package_store: self.store.key_packages.clone(),
            //     mls_server: self.mls.clone(),
            // })
            .add(GetKeyPackagesHandler {
                key_package_store: self.store.key_packages.clone(),
            })
            .add(UpdateUserMlsHandler::new(
                self.verifier.clone(),
                self.store.users.clone(),
                self.store.messages.clone(),
                self.store.key_packages.clone(),
                self.mls.clone(),
            ))
            .add(GetKeyPackagesHandler {
                key_package_store: self.store.key_packages.clone(),
            })
            .add(agent_message_handler)
            .add(client_message_handler)
            .add(agent_sender)
            .add(client_sender);

        Ok(commands)
    }
}
