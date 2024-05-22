use anyhow::Result;
use async_trait::async_trait;
use svalin_rpc::{CommandHandler, Session, SessionOpen};

use super::ServerJoinManager;

pub(super) struct JoinRequestHandler {
    manager: ServerJoinManager,
}

impl JoinRequestHandler {
    pub(super) fn new(manager: ServerJoinManager) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl CommandHandler for JoinRequestHandler {
    fn key(&self) -> String {
        "request_join".into()
    }

    async fn handle(&self, mut session: Session<SessionOpen>) -> Result<()> {
        todo!()
    }
}
