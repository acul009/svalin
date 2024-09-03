use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use anyhow::Result;
use async_trait::async_trait;
use tracing::debug;

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

    async fn handle(&self, session: &mut Session) -> anyhow::Result<()> {
        let ping: u64 = session.read_object().await?;
        session.write_object(&ping).await?;

        Ok(())
    }
}

pub struct Ping;

#[async_trait]
impl CommandDispatcher for Ping {
    type Output = Duration;

    fn key(&self) -> String {
        ping_key()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Duration> {
        let ping = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos();

        debug!("sending ping");

        session.write_object(&ping).await?;

        debug!("ping sent, waiting for pong!");

        let pong: u128 = session.read_object().await?;

        debug!("pong received");

        let now: u128 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos();

        let diff = Duration::from_nanos((now - pong).try_into()?);

        Ok(diff)
    }
}
