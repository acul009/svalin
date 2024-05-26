use anyhow::Result;
use svalin_rpc::skip_verify::SkipServerVerification;
use tracing::debug;

mod init;

use crate::shared::commands::public_server_status::get_public_statusDispatcher;

pub struct Agent {
    rpc: svalin_rpc::Client,
}

impl Agent {
    pub async fn initCmd(address: String) -> Result<()> {
        println!("===============================\nWelcome to svalin!\n===============================\nInitializing Agent...");

        debug!("try connecting to {address}");

        let client =
            svalin_rpc::Client::connect(&address, None, SkipServerVerification::new()).await?;

        debug!("successfully connected");

        let mut conn = client.upstream_connection();

        debug!("requesting public status");

        let server_status = conn.get_public_status().await?;

        debug!("public status: {server_status:?}");

        match server_status {
            crate::shared::commands::public_server_status::PublicStatus::WaitingForInit => todo!(),
            crate::shared::commands::public_server_status::PublicStatus::Ready => todo!(),
        }
        todo!()
    }
}
