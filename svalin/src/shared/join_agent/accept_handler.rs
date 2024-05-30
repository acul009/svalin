use async_trait::async_trait;
use svalin_rpc::CommandHandler;

use super::ServerJoinManager;

pub struct JoinAcceptHandler {
    manager: ServerJoinManager,
}

impl JoinAcceptHandler {
    pub(super) fn new(manager: ServerJoinManager) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl CommandHandler for JoinAcceptHandler {
    fn key(&self) -> String {
        "accept_join".to_string()
    }

    async fn handle(
        &self,
        _session: &mut svalin_rpc::Session<svalin_rpc::SessionOpen>,
    ) -> anyhow::Result<()> {
        todo!()
    }
}
