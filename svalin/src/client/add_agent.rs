use crate::shared::join_agent::accept_handler::AcceptJoin;

use super::Client;

use anyhow::Result;
use svalin_rpc::rpc::connection::Connection;
use tokio::sync::oneshot;

impl Client {
    pub async fn add_agent_with_code(
        &self,
        join_code: String,
        confirm_code: oneshot::Sender<oneshot::Sender<String>>,
    ) -> Result<()> {
        let connection = self.rpc.upstream_connection();

        let _certificate = connection
            .dispatch(AcceptJoin {
                client: &self,
                join_code,
                confirm_code,
            })
            .await?;

        Ok(())
    }
}
