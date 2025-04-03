use std::sync::Arc;

use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{
        dispatcher::{DispatcherError, TakeableCommandDispatcher},
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::{Session, SessionReadError},
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::{client::device::RemoteLiveData, permissions::Permission};

use super::{InstallationInfo, Updater};

impl From<&PermissionPrecursor<InstallationInfoHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<InstallationInfoHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

pub struct InstallationInfoHandler {
    updater: Arc<Updater>,
}

impl InstallationInfoHandler {
    pub fn new(updater: Arc<Updater>) -> Self {
        Self { updater }
    }
}

#[async_trait]
impl CommandHandler for InstallationInfoHandler {
    type Request = ();

    fn key() -> String {
        "check-update".to_string()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let mut watch = self.updater.subscribe_install_info();
        {
            let info = watch.borrow().clone();
            session.write_object(&info).await?;
        }
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = watch.changed() => {
                    let info = watch.borrow().clone();
                    session.write_object(&info).await?;
                }
            }
        }

        Ok(())
    }
}

pub struct InstallationInfoDispatcher {
    pub send: watch::Sender<RemoteLiveData<InstallationInfo>>,
}

#[derive(Debug, thiserror::Error)]
pub enum InstallationInfoError {
    #[error(transparent)]
    ReadError(#[from] SessionReadError),
}

#[async_trait]
impl TakeableCommandDispatcher for InstallationInfoDispatcher {
    type Request = ();
    type Output = ();
    type InnerError = InstallationInfoError;

    fn key() -> String {
        InstallationInfoHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
        _request: Self::Request,
    ) -> Result<Self::Output, DispatcherError<Self::InnerError>> {
        if let Some(mut session) = session.take() {
            tokio::spawn(async move {
                let send = self.send;
                loop {
                    tokio::select! {
                        _ = send.closed() => break,
                        installation_info = session.read_object::<InstallationInfo>() => {
                            if let Ok(installation_info) = installation_info {
                                let _ = send.send(RemoteLiveData::Ready(installation_info));
                            } else {
                                break;
                            }
                        }
                    }
                }
            });

            Ok(())
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}
