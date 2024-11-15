use core::time;
use std::{net::SocketAddr, sync::Arc};

use agent_store::AgentStore;
use anyhow::{anyhow, Context, Result};
use rand::{
    distributions::{self},
    thread_rng, Rng,
};
use serde::{Deserialize, Serialize};
use svalin_pki::{
    verifier::{self, KnownCertificateVerifier},
    Certificate, Keypair, PermCredentials,
};
use svalin_rpc::{
    commands::{forward::ForwardHandler, ping::PingHandler},
    permissions::{anonymous_permission_handler::AnonymousPermissionHandler, DummyPermission},
    rpc::command::handler::HandlerCollection,
    verifiers::skip_verify::SkipClientVerification,
};
use tokio::sync::mpsc;
use tracing::{debug, error};

use crate::{
    permissions::server_permission_handler::ServerPermissionHandler,
    shared::{
        commands::{
            add_user::AddUserHandler,
            agent_list::AgentListHandler,
            init::InitHandler,
            public_server_status::{PublicStatus, PublicStatusHandler},
        },
        join_agent::add_agent::AddAgentHandler,
    },
    verifier::{
        server_storage_verifier::ServerStorageVerifier, tls_optional_wrapper::TlsOptionalWrapper,
        verification_helper::VerificationHelper,
    },
};

use svalin_rpc::rpc::server::RpcServer;

use self::user_store::UserStore;

pub mod agent_store;
pub mod user_store;

#[derive(Debug)]
pub struct Server {
    rpc: RpcServer,
    scope: marmelade::Scope,
    root: Certificate,
    credentials: PermCredentials,
    user_store: Arc<UserStore>,
    agent_store: Arc<AgentStore>,
}

#[derive(Serialize, Deserialize)]
struct BaseConfig {
    root_cert: Certificate,
    credentials: Vec<u8>,
}

impl Server {
    pub async fn prepare(addr: SocketAddr, scope: marmelade::Scope) -> Result<Self> {
        let mut base_config: Option<BaseConfig> = None;

        scope.view(|b| {
            base_config = b.get_object("base_config")?;

            Ok(())
        })?;

        if base_config.is_none() {
            // initialize

            debug!("Server is not yet initialized, starting initialization routine");

            let (root, credentials) = Self::init_server(addr)
                .await
                .context("failed to initialize server")?;

            debug!("Initialisation complete, waiting for init server shutdown");

            // Sleep until the init server has shut down and released the Port
            tokio::time::sleep(time::Duration::from_secs(5)).await;

            let key = Server::get_encryption_key(&scope)?;

            let conf = BaseConfig {
                root_cert: root,
                credentials: credentials.to_bytes(key).await?,
            };

            scope.update(|b| {
                b.put_object("base_config", &conf)?;

                Ok(())
            })?;

            base_config = Some(conf);
        } else {
            debug!("Server is already initialized");
        }

        if base_config.is_none() {
            unreachable!("server init failed but continued anyway")
        }

        let base_config = base_config.expect("This should not ever happen");

        let root = base_config.root_cert;

        let credentials = PermCredentials::from_bytes(
            &base_config.credentials,
            Server::get_encryption_key(&scope)?,
        )
        .await?;

        let user_store = UserStore::open(scope.subscope("users".into())?);

        let agent_store = AgentStore::open(scope.subscope("agents".into())?, root.clone());

        let helper = VerificationHelper::new(root.clone(), user_store.clone());

        let verifier = ServerStorageVerifier::new(
            helper,
            root.clone(),
            user_store.clone(),
            agent_store.clone(),
        )
        .to_tls_verifier();

        let verifier = TlsOptionalWrapper::new(verifier);

        // TODO: proper client verification
        let rpc =
            RpcServer::new(addr, &credentials, verifier).context("failed to create rpc server")?;

        Ok(Self {
            rpc,
            scope,
            root,
            credentials,
            user_store,
            agent_store,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let permission_handler: ServerPermissionHandler =
            ServerPermissionHandler::new(self.root.clone());

        let commands = HandlerCollection::new(permission_handler);

        let join_manager = crate::shared::join_agent::ServerJoinManager::new();

        commands
            .chain()
            .await
            .add(PingHandler::new())
            .add(PublicStatusHandler::new(PublicStatus::Ready))
            .add(AddUserHandler::new(self.user_store.clone()))
            .add(join_manager.create_request_handler())
            .add(join_manager.create_accept_handler())
            .add(ForwardHandler::new(self.rpc.clone()))
            .add(AddAgentHandler::new(self.agent_store.clone())?)
            .add(AgentListHandler::new(
                self.agent_store.clone(),
                self.rpc.clone(),
            ));

        self.rpc.run(commands).await
    }

    fn get_encryption_key(scope: &marmelade::Scope) -> Result<Vec<u8>> {
        let mut saved_key: Option<Vec<u8>> = None;

        scope.view(|b| {
            if let Some(raw) = b.get_kv("server_encryption_key") {
                saved_key = Some(raw.value().to_vec());
            }

            Ok(())
        })?;

        if let Some(key) = saved_key {
            Ok(key)
        } else {
            let key: Vec<u8> = thread_rng()
                .sample_iter(distributions::Standard)
                .take(32)
                .collect();

            scope.update(|b| {
                b.put("server_encryption_key", key.clone())?;
                Ok(())
            })?;

            Ok(key)
        }
    }

    async fn init_server(addr: SocketAddr) -> Result<(Certificate, PermCredentials)> {
        let temp_credentials = Keypair::generate()?.to_self_signed_cert()?;

        let rpc = Arc::new(RpcServer::new(
            addr,
            &temp_credentials,
            SkipClientVerification::new(),
        )?);
        let rpc_clone = rpc.clone();

        let (send, mut receive) = mpsc::channel::<(Certificate, PermCredentials)>(1);

        let permission_handler = AnonymousPermissionHandler::<DummyPermission>::default();

        let commands = HandlerCollection::new(permission_handler);
        commands
            .chain()
            .await
            .add(InitHandler::new(send))
            .add(PublicStatusHandler::new(PublicStatus::WaitingForInit));

        debug!("starting up init server");

        let handle = tokio::spawn(async move { rpc.run(commands).await });

        debug!("init server running");

        if let Some(result) = receive.recv().await {
            debug!("successfully initialized server");
            rpc_clone.close();
            handle.abort();
            Ok(result)
        } else {
            error!("error when trying to initialize server");
            rpc_clone.close();
            handle.abort();
            Err(anyhow!("error initializing server"))
        }
    }

    pub fn close(&self) {
        self.rpc.close();
    }
}
