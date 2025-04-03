use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::{Session, SessionReadError},
};
use svalin_sysctl::realtime::RealtimeStatus;
use tokio::{select, sync::watch};
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::{client::device::RemoteLiveData, permissions::Permission};

#[derive(Default)]
pub struct RealtimeStatusHandler;

impl From<&PermissionPrecursor<RealtimeStatusHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<RealtimeStatusHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

#[async_trait]
impl CommandHandler for RealtimeStatusHandler {
    type Request = ();

    fn key() -> String {
        "realtime-status".into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _: Self::Request,
        cancel: CancellationToken,
    ) -> Result<()> {
        debug!("realtime status requested");
        loop {
            let status = RealtimeStatus::get().await;
            debug!("sending realtime status");
            debug!("cpu: {:?}", status.cpu);

            session.write_object(&status).await?;

            select! {
                _ = cancel.cancelled() => {
                    return Ok(());
                }
                _ = tokio::time::sleep(Duration::from_secs(2)) => {},
            }
        }
    }
}

pub struct SubscribeRealtimeStatus {
    pub send: watch::Sender<RemoteLiveData<RealtimeStatus>>,
    pub cancel: CancellationToken,
}

#[derive(Debug, thiserror::Error)]
pub enum SubscribeRealtimeStatusError {
    #[error("error reading update: {0}")]
    ReadUpdateError(SessionReadError),
}

#[async_trait]
impl CommandDispatcher for SubscribeRealtimeStatus {
    type Output = ();
    type Request = ();
    type Error = SubscribeRealtimeStatusError;

    fn key() -> String {
        RealtimeStatusHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(self, session: &mut Session, _: Self::Request) -> Result<(), Self::Error> {
        loop {
            select! {
                _ = self.cancel.cancelled() => {
                    return Ok(());
                },
                status  = session.read_object::<RealtimeStatus>() => {
                    match status {
                        Ok(status) => {
                            debug!("received realtime status");
                            if let Err(_) = self.send.send(RemoteLiveData::Ready(status)) {
                                return Ok(());
                            }
                        }
                        Err(err) => {
                            return Err(SubscribeRealtimeStatusError::ReadUpdateError(err));
                        }
                    };
                },
            }
        }
    }
}
