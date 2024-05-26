use anyhow::{anyhow, Result};
use svalin_rpc::skip_verify::SkipServerVerification;
use tracing::debug;

use crate::shared::commands::public_server_status::get_public_statusDispatcher;

use super::Agent;

impl Agent {
    pub async fn init(address: String) -> Result<()> {
        debug!("try connecting to {address}");

        let client =
            svalin_rpc::Client::connect(&address, None, SkipServerVerification::new()).await?;

        debug!("successfully connected");

        let mut conn = client.upstream_connection();

        debug!("requesting public status");

        let server_status = conn.get_public_status().await?;

        debug!("public status: {server_status:?}");

        match server_status {
            crate::shared::commands::public_server_status::PublicStatus::WaitingForInit => {
                Err(anyhow!("Server is not ready to accept agents"))
            }

            crate::shared::commands::public_server_status::PublicStatus::Ready => {
                // register agent with server first

                todo!()
            }
        }
    }
}
