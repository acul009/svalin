use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_macros::rpc_dispatch;
use svalin_rpc::{Session, SessionOpen};

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
impl svalin_rpc::CommandHandler for PublicStatusHandler {
    fn key(&self) -> String {
        public_status_key()
    }

    #[must_use]
    async fn handle(&self, session: &mut Session<SessionOpen>) -> anyhow::Result<()> {
        session.write_object(&self.current_status).await?;
        Ok(())
    }
}

#[rpc_dispatch(public_status_key())]
pub async fn get_public_status(session: &mut Session<SessionOpen>) -> Result<PublicStatus> {
    let status = session.read_object().await?;

    Ok(status)
}
