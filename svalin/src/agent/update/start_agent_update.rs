use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError},
};
use tokio_util::sync::CancellationToken;

use super::{UpdateChannel, Updater};

pub struct StartUpdateHandler {
    updater: Arc<Updater>,
}

impl StartUpdateHandler {
    pub fn new(updater: Arc<Updater>) -> Self {
        Self { updater }
    }
}

#[async_trait]
impl CommandHandler for StartUpdateHandler {
    type Request = UpdateChannel;

    fn key() -> String {
        "start_update".to_string()
    }
    async fn handle(
        &self,
        session: &mut Session,
        channel: Self::Request,
        _cancel: CancellationToken,
    ) -> Result<()> {
        let update_result = self.updater.update_to(&channel).await.map_err(|err| {
            tracing::error!("error while updating: {err}");
            ()
        });

        session.write_object(&update_result).await?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StartUpdateDispatcherError {
    #[error("error reading update response: {0}")]
    ReadError(#[from] SessionReadError),
    #[error("error while installing update")]
    UpdaterError,
}

pub struct StartUpdateDispatcher {
    pub channel: UpdateChannel,
}

impl CommandDispatcher for StartUpdateDispatcher {
    type Error = StartUpdateDispatcherError;
    type Output = ();

    type Request = UpdateChannel;
    fn key() -> String {
        StartUpdateHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &self.channel
    }

    async fn dispatch(
        self,
        session: &mut Session,
    ) -> std::result::Result<Self::Output, Self::Error> {
        let result: Result<(), ()> = session.read_object().await?;

        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(StartUpdateDispatcherError::UpdaterError),
        }
    }
}
