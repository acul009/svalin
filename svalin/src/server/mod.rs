use core::time;
use std::{net::SocketAddr, sync::Arc};

use agent_store::AgentStore;
use anyhow::{anyhow, Context, Result};
use rand::{
    distributions::{self},
    thread_rng, Rng,
};
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, Keypair, PermCredentials};
use svalin_rpc::{
    commands::{forward::ForwardHandler, ping::PingHandler},
    rpc::command::HandlerCollection,
    verifiers::skip_verify::SkipClientVerification,
};
use tokio::sync::mpsc;
use tracing::{debug, error};

use crate::shared::{
    commands::{
        add_user::AddUserHandler,
        agent_list::AgentListHandler,
        init::InitHandler,
        public_server_status::{PublicStatus, PublicStatusHandler},
    },
    join_agent::add_agent::AddAgentHandler,
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
            if let Some(raw) = b.get_kv("base_config") {
                base_config = Some(serde_json::from_slice(raw.value())?);
            }

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
            tokio::time::sleep(time::Duration::from_secs(1)).await;

            let key = Server::get_encryption_key(&scope)?;

            let conf = BaseConfig {
                root_cert: root,
                credentials: credentials.to_bytes(key).await?,
            };

            scope.update(|b| {
                let vec = serde_json::to_vec(&conf)?;
                b.put("base_config", vec)?;

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

        // TODO: proper client verification
        let rpc = RpcServer::new(addr, &credentials, SkipClientVerification::new())
            .context("failed to create rpc server")?;

        Ok(Self {
            rpc,
            scope,
            root,
            credentials,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let userstore = UserStore::open(self.scope.subscope("users".into())?);

        let agent_store = AgentStore::open(self.scope.subscope("agents".into())?);

        let commands = HandlerCollection::new();

        let join_manager = crate::shared::join_agent::ServerJoinManager::new();

        commands
            .chain()
            .await
            .add(PingHandler::new())
            .add(PublicStatusHandler::new(PublicStatus::Ready))
            .add(AddUserHandler::new(userstore))
            .add(join_manager.create_request_handler())
            .add(join_manager.create_accept_handler())
            .add(ForwardHandler::new(self.rpc.clone()))
            .add(AddAgentHandler::new(agent_store.clone())?)
            .add(AgentListHandler::new(agent_store, self.rpc.clone()));

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

        let commands = HandlerCollection::new();
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
            handle.abort();
            rpc_clone.close();
            Ok(result)
        } else {
            error!("error when trying to initialize server");
            handle.abort();
            rpc_clone.close();
            Err(anyhow!("error initializing server"))
        }
    }

    pub fn close(&self) {
        self.rpc.close();
    }
}
