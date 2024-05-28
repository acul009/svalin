use std::time::Duration;

use anyhow::{anyhow, Ok, Result};
use async_trait::async_trait;
use rand::Rng;
use svalin_macros::rpc_dispatch;
use svalin_rpc::{CommandHandler, Session, SessionOpen};
use tokio::sync::oneshot;

use super::ServerJoinManager;

pub struct JoinRequestHandler {
    manager: ServerJoinManager,
}

impl JoinRequestHandler {
    pub(super) fn new(manager: ServerJoinManager) -> Self {
        Self { manager }
    }
}

fn create_join_code() -> String {
    rand::thread_rng().gen_range(0..999999).to_string()
}

fn join_request_key() -> String {
    "join_request".to_string()
}

#[async_trait]
impl CommandHandler for JoinRequestHandler {
    fn key(&self) -> String {
        join_request_key()
    }

    async fn handle(&self, mut session: Session<SessionOpen>) -> Result<()> {
        let mut joincode = create_join_code();
        while let Err(sess) = self.manager.add_session(joincode, session).await {
            session = sess;
            tokio::time::sleep(Duration::from_secs(5)).await;

            joincode = create_join_code();
        }

        Ok(())
    }
}

#[rpc_dispatch(join_request_key())]
pub async fn request_join(
    session: &mut Session<SessionOpen>,
    join_code_channel: oneshot::Sender<String>,
    confirm_code_channel: oneshot::Sender<String>,
    success_channel: oneshot::Sender<()>,
) -> Result<()> {
    let join_code: String = session.read_object().await?;
    join_code_channel
        .send(join_code)
        .map_err(|err| anyhow!(err))?;

    tokio::time::sleep(Duration::from_secs(60)).await;

    todo!();

    Ok(())
}
