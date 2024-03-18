use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate as svalin_rpc;
use anyhow::Result;
use async_trait::async_trait;
use svalin_macros::rpc_dispatch;

use crate::{command::CommandHandler, session, SessionOpen};

pub(crate) struct PingHandler;

#[async_trait]
impl CommandHandler for PingHandler {
    fn key(&self) -> String {
        "ping".to_owned()
    }

    async fn handle(
        &self,
        mut session: session::Session<session::SessionOpen>,
    ) -> anyhow::Result<()> {
        loop {
            let ping: u64 = session.read_object().await?;
            session.write_object(&ping).await?;
        }
    }
}

#[rpc_dispatch]
async fn ping(session: &mut crate::Session<SessionOpen>) -> Result<Duration> {
    let ping = SystemTime::now();
    session.write_object(&ping).await?;

    let pong: SystemTime = session.read_object().await?;

    Ok(SystemTime::now().duration_since(pong)? )
}
