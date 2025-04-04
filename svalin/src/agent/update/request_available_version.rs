use std::sync::Arc;

use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::{Session, SessionReadError},
};
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::permissions::Permission;

use super::{UpdateChannel, Updater};

pub struct AvailableVersionHandler {
    updater: Arc<Updater>,
}

impl AvailableVersionHandler {
    pub fn new(updater: Arc<Updater>) -> Self {
        Self { updater }
    }
}

impl From<&PermissionPrecursor<AvailableVersionHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<AvailableVersionHandler>) -> Self {
        return Permission::RootOnlyPlaceholder;
    }
}

#[async_trait]
impl CommandHandler for AvailableVersionHandler {
    type Request = UpdateChannel;

    fn key() -> String {
        "check-update".to_string()
    }

    async fn handle(
        &self,
        session: &mut Session,
        channel: Self::Request,
        _: CancellationToken,
    ) -> anyhow::Result<()> {
        debug!("requested update check for {:?}", channel);
        let version = self.updater.check_channel_version(&channel).await?;

        session.write_object(&version).await?;

        Ok(())
    }
}

pub struct AvailableVersionDispatcher {
    pub channel: UpdateChannel,
}

impl AvailableVersionDispatcher {
    pub fn new(channel: UpdateChannel) -> Self {
        Self { channel }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AvailableVersionError {
    #[error("error reading data: {0}")]
    ReadError(#[from] SessionReadError),
}

#[async_trait]
impl CommandDispatcher for AvailableVersionDispatcher {
    type Request = UpdateChannel;
    type Error = AvailableVersionError;
    type Output = String;

    fn key() -> String {
        AvailableVersionHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        self.channel.clone()
    }

    async fn dispatch(
        self,
        session: &mut Session,
        _request: Self::Request,
    ) -> Result<Self::Output, Self::Error> {
        Ok(session.read_object().await?)
    }
}
