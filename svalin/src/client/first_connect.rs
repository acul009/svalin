use anyhow::Result;
use svalin_pki::PermCredentials;

use crate::init::initDispatcher;

pub enum FirstConnect {
    Init(Init),
    Login(Login),
}

struct Init {
    rpc: svalin_rpc::Client,
}

impl Init {
    pub async fn init(&self) -> Result<()> {
        let root = self.rpc.upstream_connection().init().await?;

        // create root user on server

        // save configuration to profile

        todo!()
    }
}

struct Login {
    rpc: svalin_rpc::Client,
}

impl Login {
    pub async fn login(&self) -> Result<()> {
        todo!()
    }
}
