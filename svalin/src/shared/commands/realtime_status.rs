use std::{pin::pin, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use futures::{select, FutureExt};
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::Session,
};
use svalin_sysctl::realtime::RealtimeStatus;
use tokio::sync::{oneshot, watch};
use tracing::debug;

use crate::{client::device::RemoteLiveData, permissions::Permission};

#[derive(Default)]
pub struct RealtimeStatusHandler;

impl From<&PermissionPrecursor<(), RealtimeStatusHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<(), RealtimeStatusHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

#[async_trait]
impl CommandHandler for RealtimeStatusHandler {
    type Request = ();

    fn key() -> String {
        "realtime-status".into()
    }

    async fn handle(&self, session: &mut Session, _: Self::Request) -> Result<()> {
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
    type Request = ();

    fn key() -> String {
        RealtimeStatusHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(self, session: &mut Session, _: Self::Request) -> Result<()> {
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
