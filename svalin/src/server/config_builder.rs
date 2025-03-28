use core::net::SocketAddr;

use anyhow::Result;
use tokio_util::sync::CancellationToken;

use super::{Server, ServerConfig};

pub struct ServerConfigBuilder<A, B, C> {
    addr: A,
    tree: B,
    cancel: C,
}

pub(super) fn new() -> ServerConfigBuilder<(), (), ()> {
    ServerConfigBuilder {
        addr: (),
        tree: (),
        cancel: (),
    }
}

impl<A, B, C> ServerConfigBuilder<A, B, C> {
    pub fn addr(self, addr: SocketAddr) -> ServerConfigBuilder<SocketAddr, B, C> {
        ServerConfigBuilder {
            addr: addr,
            tree: self.tree,
            cancel: self.cancel,
        }
    }

    pub fn tree(self, tree: sled::Tree) -> ServerConfigBuilder<A, sled::Tree, C> {
        ServerConfigBuilder {
            addr: self.addr,
            tree,
            cancel: self.cancel,
        }
    }

    pub fn cancel(self, cancel: CancellationToken) -> ServerConfigBuilder<A, B, CancellationToken> {
        ServerConfigBuilder {
            addr: self.addr,
            tree: self.tree,
            cancel: cancel,
        }
    }
}

impl ServerConfigBuilder<SocketAddr, sled::Tree, CancellationToken> {
    pub async fn start_server(self) -> Result<Server> {
        let config = self.to_config();

        Server::start(config).await
    }

    fn to_config(self) -> ServerConfig {
        ServerConfig {
            addr: self.addr,
            tree: self.tree,
            cancelation_token: self.cancel,
        }
    }
}
