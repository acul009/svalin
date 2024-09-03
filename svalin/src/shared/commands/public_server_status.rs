use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};

fn public_status_key() -> String {
    "public_status".to_owned()
}

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
    fn key(&self) -> String {
        public_status_key()
    }

    #[must_use]
    async fn handle(&self, session: &mut Session) -> anyhow::Result<()> {
        session.write_object(&self.current_status).await?;
        Ok(())
    }
}

pub struct GetPutblicStatus;

#[async_trait]
impl CommandDispatcher for GetPutblicStatus {
    type Output = PublicStatus;

    fn key(&self) -> String {
        public_status_key()
    }

    async fn dispatch(self, session: &mut Session) -> Result<PublicStatus> {
        let status = session.read_object().await?;

        Ok(status)
    }
}
