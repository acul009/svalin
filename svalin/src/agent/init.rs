use anyhow::{Context, anyhow};
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};
use svalin_rpc::verifiers::skip_verify::SkipServerVerification;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::shared::commands::public_server_status::GetPutblicStatus;
use crate::shared::join_agent::AgentInitPayload;
use crate::shared::join_agent::request_handler::RequestJoin;

pub async fn init(address: String, profile: &str) -> anyhow::Result<WaitingForInit> {
    if super::get_config(profile).await?.is_some() {
        return Err(anyhow!("Agent is already initialized"));
    }

    tracing::trace!("try connecting to {address}");

    let client = RpcClient::connect(
        &address,
        None,
        SkipServerVerification::new(),
        CancellationToken::new(),
    )
    .await?;

    tracing::trace!("successfully connected");

    let conn = client.upstream_connection();

    // tracing::trace!("requesting public status");

    let server_status = conn.dispatch(GetPutblicStatus).await?;

    // tracing::trace!("public status: {server_status:?}");

    match server_status {
        crate::shared::commands::public_server_status::PublicStatus::WaitingForInit => {
            Err(anyhow!("Server is not ready to accept agents"))
        }

        crate::shared::commands::public_server_status::PublicStatus::Ready => {
            // register agent with server first

            let (join_codes_send, join_codes_recv) = tokio::sync::oneshot::channel();

            let (join_success_send, join_success_recv) =
                tokio::sync::oneshot::channel::<AgentInitPayload>();

            let conn2 = client.upstream_connection();

            tokio::spawn(async move {
                match conn2
                    .dispatch(RequestJoin {
                        address,
                        join_codes_channel: join_codes_send,
                    })
                    .await
                {
                    Ok(init_payload) => {
                        join_success_send.send(init_payload).unwrap();
                    }
                    Err(err) => {
                        tracing::error!("failed to request join: {err}");
                    }
                }
            });

            let (join_code, confirm_code) = join_codes_recv.await?;

            Ok(WaitingForInit::new(
                join_code,
                confirm_code,
                join_success_recv,
            ))
        }
    }
}

pub struct WaitingForInit {
    join_code: String,
    confirm_code: String,
    success_channel: oneshot::Receiver<AgentInitPayload>,
}

impl WaitingForInit {
    fn new(
        join_code: String,
        confirm_code: String,
        success_channel: oneshot::Receiver<AgentInitPayload>,
    ) -> Self {
        Self {
            join_code,
            confirm_code,
            success_channel,
        }
    }

    pub fn join_code(&self) -> &str {
        &self.join_code
    }

    pub fn confirm_code(&self) -> &str {
        &self.confirm_code
    }

    pub async fn wait_for_init(self, profile: &str) -> anyhow::Result<()> {
        let init_data = self.success_channel.await?;

        super::init_with(init_data, profile)
            .await
            .context("error saving init data")?;

        Ok(())
    }
}
