use crate as svalin_rpc;
use crate::rpc::connection::{Connection, ConnectionBase, DirectConnection};
use crate::rpc::peer::Peer;
use crate::rpc::{
    command::CommandHandler,
    server::RpcServer,
    session::{Session, SessionOpen},
};
use crate::transport::session_transport::SessionTransport;
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

pub struct ForwardConnection<T> {
    connection: T,
    target: Certificate,
}

impl<T> ForwardConnection<T> {
    pub fn new(base_connection: T, target: Certificate) -> Self {
        Self {
            connection: base_connection,
            target: target,
        }
    }
}

#[async_trait]
impl<T> ConnectionBase for ForwardConnection<T>
where
    T: Connection,
{
    async fn open_raw_session(&self) -> Result<Box<dyn SessionTransport>> {
        let mut base_session = self.connection.open_session(forward_key()).await?;

        base_session.write_object(&self.target).await?;

        Ok(base_session.extract_transport())
    }

    /// For the ForwardConnection we always return anonymous, since there is no
    /// E2E tunnel yet, we can't guarantee, that the server didn't intercept the
    /// session.
    fn peer(&self) -> &Peer {
        &Peer::Anonymous
    }

    async fn closed(&self) {
        self.connection.closed().await
    }
}

#[rpc_dispatch(forward_key())]
pub async fn forward(session: &mut Session<SessionOpen>) -> Result<()> {
    todo!()
}
