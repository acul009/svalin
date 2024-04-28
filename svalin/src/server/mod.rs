use core::time;
use std::{net::SocketAddr, sync::Arc};

use anyhow::{anyhow, Result};
use rand::{
    distributions::{self, Distribution, Standard},
    thread_rng, Rng,
};
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, PermCredentials};
use svalin_rpc::HandlerCollection;
use tokio::sync::mpsc;

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

            let (root, credentials) = Self::init_server(addr).await?;

            // Sleep until the init server has shut down and released the Port
            tokio::time::sleep(time::Duration::from_secs(1)).await;

            let key = Server::get_encryption_key(&scope)?;

            let conf = BaseConfig {
                root_cert: root,
                credentials: credentials.to_bytes(&key)?,
            };

            scope.update(|b| {
                let vec = serde_json::to_vec(&conf)?;
                b.put("base_config", vec)?;

                Ok(())
            })?;

            base_config = Some(conf);
        }

        if base_config.is_none() {
            unreachable!("server init failed but continued anyway")
        }

        let base_config = base_config.expect("This should not ever happen");

        let root = base_config.root_cert;

        let credentials = PermCredentials::from_bytes(
            &base_config.credentials,
            &Server::get_encryption_key(&scope)?,
        )?;

        let rpc = svalin_rpc::Server::new(addr)?;

        Ok(Self {
            rpc,
            scope,
            root,
            credentials,
        })
    }

    pub async fn run(&mut self, commands: Arc<HandlerCollection>) -> Result<()> {
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
        let mut rpc = svalin_rpc::Server::new(addr)?;

        let (send, mut receive) = mpsc::channel::<(Certificate, PermCredentials)>(1);

        let commands = HandlerCollection::new();
        commands.add(InitHandler::new(send)).await;

        let debug = commands.get("init").await.unwrap();

        println!("debug: {:?}", debug.key());

        println!("starting up init server");

        let handle = tokio::spawn(async move { rpc.run(commands).await });

        println!("init server running");

        if let Some(result) = receive.recv().await {
            println!("successfully initialized server");
            handle.abort();
            Ok(result)
        } else {
            println!("error when trying to initialize server");
            handle.abort();
            Err(anyhow!("error initializing server"))
        }
    }
}

#[cfg(test)]
mod test {
    use std::net::ToSocketAddrs;

    use svalin_rpc::{
        ping::{pingDispatcher, PingHandler},
        Client, HandlerCollection, SkipServerVerification,
    };

    use crate::init::{initDispatcher, InitHandler};
    use crate::Server;

    #[tokio::test]
    async fn test_init() {
        let (send_init, recv_init) = tokio::sync::oneshot::channel::<()>();

        let server_handle = tokio::spawn(async {
            let addr = "0.0.0.0:1234".to_socket_addrs().unwrap().next().unwrap();
            // delete the test db
            std::fs::remove_file("./server_test.jammdb").unwrap_or(());
            let db = marmelade::DB::open("./server_test.jammdb").expect("failed to open client db");
            let mut server = Server::prepare(addr, db.scope("default".into()).unwrap())
                .await
                .unwrap();

            send_init.send(()).unwrap();

            let commands = HandlerCollection::new();
            commands.add(PingHandler::new()).await;

            server.run(commands).await.unwrap();
        });

        let host: url::Url = "svalin://localhost:1234".parse().unwrap();

        let init_client = Client::connect(host.clone(), None, SkipServerVerification::new())
            .await
            .unwrap();

        let mut conn = init_client.upstream_connection();
        conn.init().await.unwrap();

        recv_init.await.unwrap();

        let client = Client::connect(host, None, SkipServerVerification::new())
            .await
            .unwrap();

        let mut conn = client.upstream_connection();

        let duration = conn.ping().await.unwrap();
        println!("ping duration: {:?}", duration);

        init_client.close();

        server_handle.abort();
        let _ = server_handle.await;
    }
}
