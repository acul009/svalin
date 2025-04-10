use std::{net::SocketAddr, sync::Arc, time::Duration};

use agent_store::AgentStore;
use anyhow::{Context, Result, anyhow};
use command_builder::SvalinCommandBuilder;
use config_builder::ServerConfigBuilder;
use rand::Rng;
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, Keypair, PermCredentials, verifier::KnownCertificateVerifier};
use svalin_rpc::{
    permissions::{DummyPermission, anonymous_permission_handler::AnonymousPermissionHandler},
    rpc::{command::handler::HandlerCollection, server::Socket},
    verifiers::skip_verify::SkipClientVerification,
};
use tokio::{
    sync::mpsc,
    time::{error::Elapsed, timeout},
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{debug, error};

use crate::{
    shared::commands::{
        init::InitHandler,
        public_server_status::{PublicStatus, PublicStatusHandler},
    },
    verifier::{
        server_storage_verifier::ServerStorageVerifier, tls_optional_wrapper::TlsOptionalWrapper,
        verification_helper::VerificationHelper,
    },
};

use svalin_rpc::rpc::server::RpcServer;

use self::user_store::UserStore;

pub mod agent_store;
pub mod command_builder;
pub mod config_builder;
pub mod user_store;

#[derive(Debug)]
pub struct ServerConfig {
    addr: SocketAddr,
    tree: sled::Tree,
    cancelation_token: CancellationToken,
}

pub const INIT_SERVER_SHUTDOWN_COUNTDOWN: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct Server {
    rpc: Arc<RpcServer>,
    config: ServerConfig,
    tasks: TaskTracker,
}

#[derive(Serialize, Deserialize)]
struct BaseConfig {
    root_cert: Certificate,
    credentials: Vec<u8>,
    pseudo_data_seed: Vec<u8>,
}

impl Server {
    pub fn build() -> ServerConfigBuilder<(), (), ()> {
        config_builder::new()
    }

    async fn start(config: ServerConfig) -> Result<Self> {
        let base_config = config.tree.get("base_config")?.map(|config| {
            postcard::from_bytes::<BaseConfig>(&config).context("failed to parse base config")
        });

        debug!("creating socket");

        let socket = RpcServer::create_socket(config.addr).context("failed to create socket")?;

        let base_config = match base_config {
            Some(conf) => conf?,
            None => {
                // initialize

                debug!("Server is not yet initialized, starting initialization routine");

                let (root, credentials) =
                    Self::init_server(socket.clone(), config.cancelation_token.child_token())
                        .await
                        .context("failed to initialize server")?;

                debug!("Initialisation complete, waiting for init server shutdown");

                // Sleep until the init server has shut down and released the Port
                tokio::time::sleep(INIT_SERVER_SHUTDOWN_COUNTDOWN).await;

                let key = Server::get_encryption_key(&config.tree)?;

                let pseudo_data_seed: Vec<u8> = rand::rng()
                    .sample_iter(rand::distr::StandardUniform)
                    .take(32)
                    .collect();

                let conf = BaseConfig {
                    root_cert: root,
                    credentials: credentials.to_bytes(key).await?,
                    pseudo_data_seed,
                };

                config
                    .tree
                    .insert("base_config", postcard::to_extend(&conf, Vec::new())?)?;

                config.tree.flush_async().await?;

                conf
            }
        };

        let root = base_config.root_cert;

        let credentials = PermCredentials::from_bytes(
            &base_config.credentials,
            Server::get_encryption_key(&config.tree)?,
        )
        .await?;

        let user_store = UserStore::open(config.tree.clone());

        let agent_store = AgentStore::open(config.tree.clone(), root.clone());

        let helper = VerificationHelper::new(root.clone(), user_store.clone());

        let verifier = ServerStorageVerifier::new(
            helper,
            root.clone(),
            user_store.clone(),
            agent_store.clone(),
        )
        .to_tls_verifier();

        let verifier = TlsOptionalWrapper::new(verifier);

        let command_builder = SvalinCommandBuilder {
            root_cert: root.clone(),
            server_cert: credentials.get_certificate().clone(),
            user_store: user_store,
            agent_store: agent_store,
        };

        let tasks = TaskTracker::new();

        let rpc = RpcServer::build()
            .credentials(credentials.clone())
            .client_cert_verifier(verifier)
            .cancellation_token(config.cancelation_token.clone())
            .commands(command_builder)
            .task_tracker(tasks.clone())
            .start_server(socket)
            .await?;

        Ok(Self { config, rpc, tasks })
    }

    fn get_encryption_key(tree: &sled::Tree) -> Result<Vec<u8>> {
        let key = "server_encryption_key";
        let saved_encryption_key = tree.get(key)?.map(|v| v.to_vec());

        if let Some(key) = saved_encryption_key {
            Ok(key)
        } else {
            let encryption_key: Vec<u8> = rand::rng()
                .sample_iter(rand::distr::StandardUniform)
                .take(32)
                .collect();

            tree.insert(key, encryption_key.clone())?;

            Ok(encryption_key)
        }
    }

    async fn init_server(
        socket: Socket,
        cancel: CancellationToken,
    ) -> Result<(Certificate, PermCredentials)> {
        let (send, mut receive) = mpsc::channel::<(Certificate, PermCredentials)>(1);

        let permission_handler = AnonymousPermissionHandler::<DummyPermission>::default();

        let commands = HandlerCollection::new(permission_handler);
        commands
            .chain()
            .await
            .add(InitHandler::new(send))
            .add(PublicStatusHandler::new(PublicStatus::WaitingForInit));

        let temp_credentials = Keypair::generate().to_self_signed_cert()?;

        debug!("starting up init server");
        let rpc = RpcServer::build()
            .credentials(temp_credentials)
            .cancellation_token(cancel)
            .client_cert_verifier(SkipClientVerification::new())
            .commands(commands)
            .task_tracker(TaskTracker::new())
            .start_server(socket)
            .await?;

        debug!("init server running");

        if let Some(result) = receive.recv().await {
            debug!("successfully initialized server");
            rpc.close(Duration::from_secs(1)).await?;
            Ok(result)
        } else {
            error!("error when trying to initialize server");
            rpc.close(Duration::from_secs(1)).await?;
            Err(anyhow!("error initializing server"))
        }
    }

    pub async fn close(&self, timeout_duration: Duration) -> Result<(), Elapsed> {
        self.config.cancelation_token.cancel();
        let result1 = self.rpc.close(timeout_duration).await;

        self.tasks.close();

        let result2 = timeout(timeout_duration, self.tasks.wait()).await;

        self.config
            .tree
            .flush_async()
            .await
            .err()
            .map(|err| error!("{}", err));

        match result1 {
            Err(e) => Err(e),
            Ok(()) => result2,
        }
    }
}
