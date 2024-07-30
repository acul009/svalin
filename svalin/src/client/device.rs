use std::time::Duration;

use anyhow::Result;
use svalin_pki::Certificate;
use svalin_rpc::{
    commands::{forward::ForwardConnection, ping::pingDispatcher},
    rpc::connection::{self, DirectConnection},
};

pub struct Device {
    connection: ForwardConnection<DirectConnection>,
    certificate: Certificate,
}

impl Device {
    pub fn new(connection: ForwardConnection<DirectConnection>, certificate: Certificate) -> Self {
        return Self {
            connection,
            certificate,
        };
    }

    pub async fn ping(&self) -> Result<Duration> {
        self.connection.ping().await
    }
}
