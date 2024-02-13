use svalin_pki::Keypair;
use svalin_rpc::{CommandHandler, Session, SessionOpen};

use async_trait::async_trait;

pub(crate) struct InitHandler {}

#[async_trait]
impl CommandHandler for InitHandler {
    async fn handle(&self, mut session: Session<SessionOpen>) -> anyhow::Result<()> {
        let keypair = Keypair::generate()?;
        let public_key = keypair.public_key();
        session.write_object(public_key).await?;

        todo!()
    }

    fn key(&self) -> String {
        "init".into()
    }
}
