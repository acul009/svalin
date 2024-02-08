use std::net::SocketAddr;

use anyhow::Result;

pub struct Server {
    rpc_server: svalin_rpc::Server,
    scope: marmelade::Scope,
}

impl Server {
    pub fn new(addr: SocketAddr, scope: marmelade::Scope) -> Result<Self> {
        let mut ready = false;

        scope.view(|b| {
            if let Some(_) = b.get("initialized") {
                ready = true;
            }

            Ok(())
        })?;

        if !ready {
            Server::init_server(addr, scope.clone())?;
        }

        let rpc = svalin_rpc::Server::new(addr)?;

        Ok(Self {
            rpc_server: rpc,
            scope: scope,
        })
    }

    fn init_server(addr: SocketAddr, scope: marmelade::Scope) -> Result<()> {
        let rpc = svalin_rpc::Server::new(addr)?;

        todo!()
    }
}
