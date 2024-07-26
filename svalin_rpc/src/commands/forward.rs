use crate as svalin_rpc;
use crate::rpc::connection::{Connection, ConnectionBase, DirectConnection};
use crate::rpc::{
    command::CommandHandler,
    server::RpcServer,
    session::{Session, SessionOpen},
};
use anyhow::Result;
use async_trait::async_trait;
use svalin_macros::rpc_dispatch;
use svalin_pki::Certificate;

fn forward_key() -> String {
    "public_status".to_owned()
}

pub struct ForwardHandler {
    server: RpcServer,
}

impl ForwardHandler {
    pub fn new(server: RpcServer) -> Self {
        Self { server }
    }
}

#[async_trait]
impl CommandHandler for ForwardHandler {
    fn key(&self) -> String {
        forward_key()
    }

    #[must_use]
    async fn handle(&self, session: &mut Session<SessionOpen>) -> anyhow::Result<()> {
        let target: Certificate = session.read_object().await?;

        let mut transport = self.server.open_raw_session_with(target).await?;

        session.forward_transport(&mut transport).await
    }
}

struct ForwardConnection<T> {
    connection: T,
}

// TODO
// #[async_trait]
// impl<T> ConnectionBase for ForwardConnection<T> where T: Connection {}

#[rpc_dispatch(forward_key())]
pub async fn forward(session: &mut Session<SessionOpen>) -> Result<()> {
    todo!()
}
