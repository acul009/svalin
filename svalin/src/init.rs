use anyhow::Result;
use svalin_pki::Keypair;
use svalin_rpc::{CommandHandler, Connection, Session, SessionOpen};

use async_trait::async_trait;

pub(crate) struct InitHandler {}

#[async_trait]
impl CommandHandler for InitHandler {
    async fn handle(&self, mut session: Session<SessionOpen>) -> anyhow::Result<()> {
        let keypair = Keypair::generate()?;
        let request = keypair.generate_request()?;
        session.write_object(&request).await?;

        todo!()
    }

    fn key(&self) -> String {
        "init".into()
    }
}

#[rpc_dispatch("init_key")]
async fn init(
    session: Session<SessionOpen>,
    initname: String,
    moreargs: Foo,
) -> Result<ReturnType> {
    todo!()
}

#[async_trait]
trait InitDispatcher {
    async fn init<'a>(&'a self, initname: String, moreargs: Foo) -> Result<ReturnType>;
}

#[async_trait]
impl InitDispatcher for Connection {
    async fn init<'a>(&'a self, initname: String, moreargs: Foo) -> Result<ReturnType> {
        let session = self.open_session("init_key".into()).await?;

        init(session, initname, moreargs).await
    }
}
