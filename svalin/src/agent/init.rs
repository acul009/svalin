use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};
use svalin_rpc::verifiers::skip_verify::SkipServerVerification;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::shared::commands::public_server_status::GetPutblicStatus;
use crate::shared::join_agent::AgentInitPayload;
use crate::shared::join_agent::request_handler::RequestJoin;

use super::Agent;

impl Agent {
    pub async fn init(address: String) -> Result<WaitingForInit> {
        if Self::get_config().await?.is_some() {
            return Err(anyhow!("Agent is already initialized"));
        }

        debug!("try connecting to {address}");

        let client = RpcClient::connect(
            &address,
            None,
            SkipServerVerification::new(),
            CancellationToken::new(),
        )
        .await?;

        debug!("successfully connected");

        let conn = client.upstream_connection();

        debug!("requesting public status");

        let server_status = conn.dispatch(GetPutblicStatus).await?;

        debug!("public status: {server_status:?}");

        match server_status {
            crate::shared::commands::public_server_status::PublicStatus::WaitingForInit => {
                Err(anyhow!("Server is not ready to accept agents"))
            }

            crate::shared::commands::public_server_status::PublicStatus::Ready => {
                // register agent with server first

                let (join_code_send, join_code_recv) = tokio::sync::oneshot::channel::<String>();

                let (confirm_code_send, confirm_code_recv) =
                    tokio::sync::oneshot::channel::<String>();

                let (join_success_send, join_success_recv) =
                    tokio::sync::oneshot::channel::<AgentInitPayload>();

                let conn2 = client.upstream_connection();

                tokio::spawn(async move {
                    match conn2
                        .dispatch(RequestJoin {
                            address,
                            join_code_channel: join_code_send,
                            confirm_code_channel: confirm_code_send,
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

                let join_code = join_code_recv.await?;

                Ok(WaitingForInit::new(
                    join_code,
                    confirm_code_recv,
                    join_success_recv,
                ))
            }
        }
    }
}

pub struct WaitingForInit {
    join_code: String,
    confirm_channel: oneshot::Receiver<String>,
    success_channel: oneshot::Receiver<AgentInitPayload>,
}

impl WaitingForInit {
    fn new(
        join_code: String,
        confirm_channel: oneshot::Receiver<String>,
        success_channel: oneshot::Receiver<AgentInitPayload>,
    ) -> Self {
        Self {
            join_code,
            confirm_channel,
            success_channel,
        }
    }

    pub fn join_code(&self) -> &str {
        &self.join_code
    }

    pub async fn wait_for_init(self) -> Result<WaitForConfirm> {
        let confirm_code = self.confirm_channel.await?;

        Ok(WaitForConfirm {
            join_code: self.join_code,
            confirm_code,
            success_channel: self.success_channel,
        })
    }
}

pub struct WaitForConfirm {
    join_code: String,
    confirm_code: String,
    success_channel: oneshot::Receiver<AgentInitPayload>,
}

impl WaitForConfirm {
    pub fn join_code(&self) -> &str {
        &self.join_code
    }

    pub fn confirm_code(&self) -> &str {
        &self.confirm_code
    }

    pub async fn wait_for_confirm(self) -> Result<Agent> {
        let init_data = self.success_channel.await?;

        Agent::init_with(init_data)
            .await
            .context("error saving init data")?;

        tokio::time::sleep(Duration::from_secs(3)).await;

        let agent = Agent::open(CancellationToken::new())
            .await
            .context("error opening agent after init")?;

        Ok(agent)
    }
}
