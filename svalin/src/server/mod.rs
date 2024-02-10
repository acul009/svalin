use std::net::SocketAddr;

use anyhow::Result;
use serde::{Deserialize, Serialize};


pub struct Server {
    rpc: svalin_rpc::Server,
    scope: marmelade::Scope,
}

#[derive(Serialize, Deserialize)]
struct BaseConfig {

}

impl Server {
    pub fn new(addr: SocketAddr, scope: marmelade::Scope) -> Result<Self> {
        let mut base_config: Option<BaseConfig> = None;

        scope.view(|b| {
            if let Some(raw) = b.get_kv("base_config") {
                base_config = Some(serde_json::from_slice(raw.value())?)
            }

            Ok(())
        })?;

        if base_config.is_none() {
            // initialize
            
            let conf  = Self::init_server(addr)?;
            scope.update(|b| {
                let vec = serde_json::to_vec(&conf)?;
                b.put("base_config", vec)?;

                Ok(())
            })?;
        }

        if base_config.is_none() {
            unreachable!("server init failed but continued anyway")
        }

        let rpc = svalin_rpc::Server::new(addr)?;

        Ok(Self {
            rpc,
            scope,
        })
    }

    fn init_server(addr: SocketAddr) -> Result<BaseConfig> {
        let rpc = svalin_rpc::Server::new(addr)?;

        

        todo!()
    }
}
