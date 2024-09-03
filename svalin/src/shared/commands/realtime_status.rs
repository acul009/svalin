use std::{pin::pin, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use futures::{select, FutureExt};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
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

    async fn handle(&self, session: &mut Session) -> Result<()> {
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

pub struct SubscribeRealtimeStatus {
    pub send: watch::Sender<RemoteLiveData<RealtimeStatus>>,
    pub stop: oneshot::Receiver<()>,
}

#[async_trait]
impl CommandDispatcher for SubscribeRealtimeStatus {
    type Output = ();

    fn key(&self) -> String {
        realtime_status_key()
    }

    async fn dispatch(self, session: &mut Session) -> Result<()> {
        let mut stop = pin!(self.stop.fuse());
        loop {
            select! {
                _ = stop => {
                    return Ok(());
                },
                status  = session.read_object::<RealtimeStatus>().fuse() => {
                    match status {
                        Ok(status) => {
                            debug!("received realtime status");
                            if let Err(_) = self.send.send(RemoteLiveData::Ready(status)) {
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
}
