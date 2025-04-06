use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError},
};
use tokio_util::sync::CancellationToken;

#[derive(Serialize, Deserialize, Debug)]
pub enum PublicStatus {
    WaitingForInit,
    Ready,
}

pub struct PublicStatusHandler {
    current_status: PublicStatus,
}

impl PublicStatusHandler {
    pub fn new(current_status: PublicStatus) -> Self {
        Self { current_status }
    }
}

#[async_trait]
impl CommandHandler for PublicStatusHandler {
    type Request = ();

    fn key() -> String {
        "public_status".to_owned()
    }

    #[must_use]
    async fn handle(
        &self,
        session: &mut Session,
        _: Self::Request,
        _: CancellationToken,
    ) -> anyhow::Result<()> {
        session.write_object(&self.current_status).await?;
        Ok(())
    }
}

pub struct GetPutblicStatus;

#[derive(Debug, thiserror::Error)]
pub enum GetPutblicStatusError {
    #[error("error reading status: {0}")]
    ReadStatusError(SessionReadError),
}

impl CommandDispatcher for GetPutblicStatus {
    type Output = PublicStatus;
    type Request = ();
    type Error = GetPutblicStatusError;

    fn key() -> String {
        PublicStatusHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(self, session: &mut Session) -> Result<PublicStatus, Self::Error> {
        let status = session
            .read_object()
            .await
            .map_err(GetPutblicStatusError::ReadStatusError)?;

        Ok(status)
    }
}
