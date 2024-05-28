use anyhow::{anyhow, Result};
use svalin_rpc::skip_verify::SkipServerVerification;
use tracing::debug;

use crate::shared::{
    commands::public_server_status::get_public_statusDispatcher,
    join_agent::request_handler::request_joinDispatcher,
};

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

                let (join_code_send, join_code_recv) = tokio::sync::oneshot::channel::<String>();

                let (confirm_code_send, confirm_code_recv) =
                    tokio::sync::oneshot::channel::<String>();

                let (join_success_send, join_success_recv) = tokio::sync::oneshot::channel::<()>();

                let mut conn2 = client.upstream_connection();

                tokio::spawn(async move {
                    if let Err(e) = conn2
                        .request_join(join_code_send, confirm_code_send, join_success_send)
                        .await
                    {
                        tracing::error!("failed to request join: {e:?}");
                    }
                });

                todo!()
            }
        }
    }
}

struct CachedOneShot<T>(CachedOneShotEnum<T>);

enum CachedOneShotEnum<T> {
    Channel(tokio::sync::oneshot::Receiver<T>),
    Value(T),
}

struct Joining {
    join_code: Option<String>,
    confirm_code: Option<String>,
    join_channel: Option<tokio::sync::oneshot::Receiver<String>>,
    confirm_channel: Option<tokio::sync::oneshot::Receiver<String>>,
    success_channel: tokio::sync::oneshot::Receiver<()>,
}

impl Joining {
    fn new(
        join_channel: tokio::sync::oneshot::Receiver<String>,
        confirm_channel: tokio::sync::oneshot::Receiver<String>,
        success_channel: tokio::sync::oneshot::Receiver<()>,
    ) -> Self {
        Self {
            join_code: None,
            confirm_code: None,
            join_channel: Some(join_channel),
            confirm_channel: Some(confirm_channel),
            success_channel,
        }
    }

    pub async fn get_join_code(&mut self) -> Result<String> {
        if self.join_code.is_none() {
            self.join_code = Some(self.join_channel.take().unwrap().await?);
        }
        Ok(self.join_code.as_ref().unwrap().clone())
    }

    pub async fn get_confirm_code(&mut self) -> Result<String> {
        if self.confirm_code.is_none() {
            self.confirm_code = Some(self.confirm_channel.take().unwrap().await?);
        }
        Ok(self.confirm_code.as_ref().unwrap().clone())
    }

    async fn success(self) -> Result<()> {
        self.success_channel.await?;

        Ok(())
    }
}
