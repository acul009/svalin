use crate as svalin_rpc;
use anyhow::Result;
use async_trait::async_trait;
use futures::future::ok;
use svalin_macros::rpc_dispatch;

use crate::rpc::{
    command::CommandHandler,
    session::{Session, SessionOpen},
};

struct TlsCommandHandler {}

fn tls_test_key() -> String {
    "tls_test".into()
}

#[async_trait]
impl CommandHandler for TlsCommandHandler {
    fn key(&self) -> String {
        tls_test_key()
    }

    async fn handle(&self, session: &mut Session<SessionOpen>) -> anyhow::Result<()> {
        todo!()
    }
}

#[rpc_dispatch(tls_test_key())]
pub async fn tls_test(session: &mut Session<SessionOpen>) -> Result<()> {
    session.replace_transport(|direct_transport| async { todo!() });

    Ok(())
}
