use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    self as svalin_rpc,
    rpc::{
        command::CommandHandler,
        session::{Session, SessionOpen},
    },
};
use anyhow::Result;
use async_trait::async_trait;
use svalin_macros::rpc_dispatch;

pub struct PingHandler;

impl Default for PingHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl PingHandler {
    pub fn new() -> Self {
        Self
    }
}

fn ping_key() -> String {
    "ping".to_owned()
}

#[async_trait]
impl CommandHandler for PingHandler {
    fn key(&self) -> String {
        ping_key()
    }

    async fn handle(&self, session: &mut Session<SessionOpen>) -> anyhow::Result<()> {
        let ping: u64 = session.read_object().await?;
        session.write_object(&ping).await?;

        Ok(())
    }
}

#[rpc_dispatch(ping_key())]
pub async fn ping(session: &mut Session<SessionOpen>) -> Result<Duration> {
    let ping = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos();

    session.write_object(&ping).await?;

    let pong: u128 = session.read_object().await?;

    let now: u128 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos();

    let diff = Duration::from_nanos((now - pong).try_into()?);

    Ok(diff)
}
