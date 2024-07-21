use std::{future::Future, time::Duration};

use anyhow::{anyhow, Result};
use svalin_pki::Certificate;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::skip_verify::SkipServerVerification;
use tokio::sync::oneshot;
use tracing::debug;

use crate::shared::{
    commands::public_server_status::get_public_statusDispatcher,
    join_agent::{request_handler::request_joinDispatcher, AgentInitPayload},
};

use super::Agent;

impl Agent {
    pub async fn init(address: String) -> Result<WaitingForInit> {
        debug!("try connecting to {address}");

        let client = RpcClient::connect(&address, None, SkipServerVerification::new()).await?;

        debug!("successfully connected");

        let conn = client.upstream_connection();

        debug!("requesting public status");

        let server_status = conn.get_public_status().await?;

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
                        .request_join(address, join_code_send, confirm_code_send)
                        .await
                    {
                        Ok(init_payload) => {
                            join_success_send.send(init_payload).unwrap();
                        }
                        Err(err) => {
                            tracing::error!("failed to request join: {err:?}");
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

        Agent::init_with(init_data).await?;

        tokio::time::sleep(Duration::from_secs(3)).await;

        let agent = Agent::open().await?;

        Ok(agent)
    }
}
