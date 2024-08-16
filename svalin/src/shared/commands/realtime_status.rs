use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use svalin_macros::rpc_dispatch;
use svalin_rpc::rpc::{
    command::CommandHandler,
    session::{Session, SessionOpen},
};
use svalin_sysctl::realtime::RealtimeStatus;
use tokio::sync::watch;

fn realtime_status_key() -> String {
    "realtime-status".into()
}

pub struct RealtimeStatusHandler {}

#[async_trait]
impl CommandHandler for RealtimeStatusHandler {
    fn key(&self) -> String {
        realtime_status_key()
    }

    async fn handle(&self, session: &mut Session<SessionOpen>) -> Result<()> {
        loop {
            let status = RealtimeStatus::get().await;
            session.write_object(&status).await?;

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}

#[rpc_dispatch(realtime_status_key())]
pub async fn subscribe_realtime_status(
    session: &mut Session<SessionOpen>,
    send: watch::Sender<Option<RealtimeStatus>>,
) -> Result<()> {
    loop {
        let status: Result<RealtimeStatus> = session.read_object().await;
        match status {
            Ok(status) => {
                send.send(Some(status));
            }
            Err(err) => {
                send.send(None);
                return Err(err);
            }
        }
    }
}
