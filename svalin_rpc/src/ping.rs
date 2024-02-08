use async_trait::async_trait;

use crate::{command::CommandHandler, session};

pub(crate) struct PingHandler {}

#[async_trait]
impl CommandHandler for PingHandler {
    fn key(&self) -> String {
        "ping".to_owned()
    }

    async fn handle(
        &mut self,
        mut session: session::Session<session::SessionOpen>,
    ) -> anyhow::Result<()> {
        loop {
            let ping: u64 = session.read_object().await?;
            session.write_object(&ping).await?;
        }
        Ok(())
    }
}

pub(crate) struct PingDispatcher {}
