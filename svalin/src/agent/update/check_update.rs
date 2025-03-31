use std::sync::Arc;

use async_trait::async_trait;
use svalin_rpc::rpc::{command::handler::CommandHandler, session::Session};
use tokio_util::sync::CancellationToken;

use crate::agent::update::UpdateChannel;

use super::{UpdateMethod, Updater};

pub struct CheckUpdateHandler {
    current_version: String,
    updater: Arc<Updater>,
}

#[async_trait]
impl CommandHandler for CheckUpdateHandler {
    type Request = ();

    fn key() -> String {
        "check-update".to_string()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let channel: UpdateChannel = session.read_object().await?;

        todo!()
    }
}
