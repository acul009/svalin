use std::fmt::Debug;

use crate::shared::join_agent::{accept_handler::AcceptJoin, add_agent::AddAgent};

use super::Client;

use anyhow::{Result, anyhow};
use svalin_pki::Certificate;
use svalin_rpc::rpc::connection::{
    Connection, ConnectionDispatchError, direct_connection::DirectConnection,
};
use tokio::sync::oneshot;
use tracing::debug;

impl Client {
    pub async fn add_agent_with_code(&self, join_code: String) -> Result<WaitingForConfirmCode> {
        let connection = self.rpc.upstream_connection();

        let root = self.root_certificate.clone();
        let upstream = self.upstream_certificate.clone();
        let credentials = self.user_credential.clone();

        let (wait_for_confirm_send, wait_for_confirm_recv) = oneshot::channel::<Result<()>>();

        let (confirm_code_send, confirm_code_recv) = oneshot::channel::<String>();

        let (result_send, result_recv) =
            oneshot::channel::<Result<Certificate, ConnectionDispatchError<anyhow::Error>>>();

        tokio::spawn(async move {
            let result = connection
                .dispatch(AcceptJoin {
                    join_code,
                    waiting_for_confirm: wait_for_confirm_send,
                    confirm_code_channel: confirm_code_recv,
                    credentials: &credentials,
                    root: &root,
                    upstream: &upstream,
                })
                .await;

            result_send.send(result).unwrap();
        });

        wait_for_confirm_recv.await??;

        Ok(WaitingForConfirmCode {
            confirm_code_send,
            result_revc: result_recv,
            connection: self.rpc.upstream_connection(),
        })
    }
}

pub struct WaitingForConfirmCode {
    connection: DirectConnection,
    confirm_code_send: oneshot::Sender<String>,
    result_revc: oneshot::Receiver<Result<Certificate, ConnectionDispatchError<anyhow::Error>>>,
}

impl Debug for WaitingForConfirmCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaitingForConfirmCode").finish()
    }
}

impl WaitingForConfirmCode {
    pub async fn confirm(self, confirm_code: String) -> Result<()> {
        self.confirm_code_send.send(confirm_code).unwrap();
        let certificate = self.result_revc.await?.map_err(|err| anyhow!(err))?;

        debug!("agent certificate successfully created and sent");

        self.connection
            .dispatch(AddAgent::new(&certificate))
            .await
            .map_err(|err| anyhow!(err))?;

        debug!("agent is registered on server");

        Ok(())
    }
}
