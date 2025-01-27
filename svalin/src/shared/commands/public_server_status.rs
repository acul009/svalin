use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
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

#[async_trait]
impl CommandDispatcher for GetPutblicStatus {
    type Output = PublicStatus;
    type Request = ();

    fn key() -> String {
        PublicStatusHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(self, session: &mut Session, _: Self::Request) -> Result<PublicStatus> {
        let status = session.read_object().await?;

        Ok(status)
    }
}
