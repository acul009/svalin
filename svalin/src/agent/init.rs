use std::future::Future;

use anyhow::{anyhow, Result};
use svalin_rpc::skip_verify::SkipServerVerification;
use tokio::sync::oneshot;
use tracing::debug;

use crate::shared::{
    commands::public_server_status::get_public_statusDispatcher,
    join_agent::request_handler::request_joinDispatcher,
};

use super::Agent;

impl Agent {
    pub async fn init(address: String) -> Result<WaitingForInit> {
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

struct CachedOneShot<T>(CachedOneShotEnum<T>);

pub struct WaitingForInit {
    join_code: String,
    confirm_channel: oneshot::Receiver<String>,
    success_channel: oneshot::Receiver<()>,
}

impl WaitingForInit {
    fn new(
        join_code: String,
        confirm_channel: oneshot::Receiver<String>,
        success_channel: oneshot::Receiver<()>,
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

        todo!();
    }
}

struct WaitForConfirm {
    join_code: String,
    confirm_code: String,
    success_channel: oneshot::Receiver<()>,
}

impl Future for WaitingForInit {
    type Output = String;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        todo!()
    }
}

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
