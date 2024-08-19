use std::{pin::pin, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use futures::{select, FutureExt};
use svalin_macros::rpc_dispatch;
use svalin_rpc::rpc::{
    command::CommandHandler,
    session::{Session, SessionOpen},
};
use svalin_sysctl::realtime::RealtimeStatus;
use tokio::sync::{oneshot, watch};
use tracing::debug;

use crate::client::device::RemoteLiveData;

fn realtime_status_key() -> String {
    "realtime-status".into()
}

pub struct RealtimeStatusHandler {}

impl RealtimeStatusHandler {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl CommandHandler for RealtimeStatusHandler {
    fn key(&self) -> String {
        realtime_status_key()
    }

    async fn handle(&self, session: &mut Session<SessionOpen>) -> Result<()> {
        debug!("realtime status requested");
        loop {
            let status = RealtimeStatus::get().await;
            debug!("sending realtime status");
            debug!("cpu: {:?}", status.cpu);
            session.write_object(&status).await?;

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

#[rpc_dispatch(realtime_status_key())]
pub async fn subscribe_realtime_status(
    session: &mut Session<SessionOpen>,
    send: &watch::Sender<RemoteLiveData<RealtimeStatus>>,
    stop: oneshot::Receiver<()>,
) -> Result<()> {
    let mut stop = pin!(stop.fuse());
    loop {
        select! {
            _ = stop => {
                return Ok(());
            },
            status  = session.read_object::<RealtimeStatus>().fuse() => {
                match status {
                    Ok(status) => {
                        if let Err(_) = send.send(RemoteLiveData::Ready(status)) {
                            return Ok(());
                        }
                    }
                    Err(err) => {
                        return Err(err);
                    }
                };
            },
        }
    }
}
