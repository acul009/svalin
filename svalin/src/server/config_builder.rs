use core::net::SocketAddr;

use anyhow::Result;
use tokio_util::sync::CancellationToken;

use super::{Server, ServerConfig};

pub struct ServerConfigBuilder<A, B> {
    addr: A,
    cancel: B,
}

pub(super) fn new() -> ServerConfigBuilder<(), ()> {
    ServerConfigBuilder {
        addr: (),
        cancel: (),
    }
}

impl<A, B> ServerConfigBuilder<A, B> {
    pub fn addr(self, addr: SocketAddr) -> ServerConfigBuilder<SocketAddr, B> {
        ServerConfigBuilder {
            addr,
            cancel: self.cancel,
        }
    }

    pub fn cancel(self, cancel: CancellationToken) -> ServerConfigBuilder<A, CancellationToken> {
        ServerConfigBuilder {
            addr: self.addr,
            cancel,
        }
    }
}

impl ServerConfigBuilder<SocketAddr, CancellationToken> {
    pub async fn start_server(self) -> Result<Server> {
        let config = self.to_config();

        Server::start(config).await
    }

    fn to_config(self) -> ServerConfig {
        ServerConfig {
            addr: self.addr,
            cancelation_token: self.cancel,
        }
    }
}
