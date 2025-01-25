use core::net::SocketAddr;

use anyhow::Result;
use tokio_util::sync::CancellationToken;

use super::{Server, ServerConfig};

pub struct ServerConfigBuilder<A, B, C> {
    addr: A,
    scope: B,
    cancel: C,
}

pub(super) fn new() -> ServerConfigBuilder<(), (), ()> {
    ServerConfigBuilder {
        addr: (),
        scope: (),
        cancel: (),
    }
}

impl<A, B, C> ServerConfigBuilder<A, B, C> {
    pub fn addr(self, addr: SocketAddr) -> ServerConfigBuilder<SocketAddr, B, C> {
        ServerConfigBuilder {
            addr: addr,
            scope: self.scope,
            cancel: self.cancel,
        }
    }

    pub fn scope(self, scope: marmelade::Scope) -> ServerConfigBuilder<A, marmelade::Scope, C> {
        ServerConfigBuilder {
            addr: self.addr,
            scope: scope,
            cancel: self.cancel,
        }
    }

    pub fn cancel(self, cancel: CancellationToken) -> ServerConfigBuilder<A, B, CancellationToken> {
        ServerConfigBuilder {
            addr: self.addr,
            scope: self.scope,
            cancel: cancel,
        }
    }
}

impl ServerConfigBuilder<SocketAddr, marmelade::Scope, CancellationToken> {
    pub async fn start_server(self) -> Result<Server> {
        let config = self.to_config();

        Server::start(config).await
    }

    fn to_config(self) -> ServerConfig {
        ServerConfig {
            addr: self.addr,
            scope: self.scope,
            cancelation_token: self.cancel,
        }
    }
}
