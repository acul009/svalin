use std::net::SocketAddr;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, PermCredentials};
use svalin_rpc::HandlerCollection;
use tokio::sync::oneshot;

use crate::init::InitHandler;

pub struct Server {
    rpc: svalin_rpc::Server,
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
    pub async fn run(addr: SocketAddr, scope: marmelade::Scope) -> Result<Self> {
        let mut base_config: Option<BaseConfig> = None;

        scope.view(|b| {
            if let Some(raw) = b.get_kv("base_config") {
                base_config = Some(serde_json::from_slice(raw.value())?)
            }

            Ok(())
        })?;

        if base_config.is_none() {
            // initialize

            let (root, credentials) = Self::init_server(addr).await?;
            let key = Server::get_encryption_key(&scope);

            let conf = BaseConfig {
                root_cert: root,
                credentials: credentials.to_bytes(&key)?,
            };

            scope.update(|b| {
                let vec = serde_json::to_vec(&conf)?;
                b.put("base_config", vec)?;

                Ok(())
            })?;
        }

        if base_config.is_none() {
            unreachable!("server init failed but continued anyway")
        }

        let base_config = base_config.expect("This should not ever happen");

        let root = base_config.root_cert;

        let credentials = PermCredentials::from_bytes(
            &base_config.credentials,
            &Server::get_encryption_key(&scope),
        )?;

        let rpc = svalin_rpc::Server::new(addr)?;

        Ok(Self {
            rpc,
            scope,
            root,
            credentials,
        })
    }

    fn get_encryption_key(scope: &marmelade::Scope) -> Vec<u8> {
        todo!()
    }

    async fn init_server(addr: SocketAddr) -> Result<(Certificate, PermCredentials)> {
        let mut rpc = svalin_rpc::Server::new(addr)?;

        let (send, receive) = oneshot::channel::<(Certificate, PermCredentials)>();

        let commands = HandlerCollection::new();
        commands.add(InitHandler::new(send));

        let handle = tokio::spawn(async move { rpc.run(commands).await });

        let result = receive.await?;

        handle.abort();

        Ok(result)
    }
}
