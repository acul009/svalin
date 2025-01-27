use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use anyhow::Result;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::debug;

#[derive(Default)]
pub struct PingHandler;

fn ping_key() -> String {
    "ping".to_owned()
}

#[async_trait]
impl CommandHandler for PingHandler {
    type Request = ();

    fn key() -> String {
        ping_key()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _: Self::Request,
        _: CancellationToken,
    ) -> anyhow::Result<()> {
        let ping: u64 = session.read_object().await?;
        session.write_object(&ping).await?;

        Ok(())
    }
}

pub struct Ping;

#[async_trait]
impl CommandDispatcher for Ping {
    type Output = Duration;

    type Request = ();

    fn key() -> String {
        ping_key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(self, session: &mut Session, _: Self::Request) -> Result<Duration> {
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
