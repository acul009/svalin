use crate::shared::join_agent::accept_handler::accept_joinDispatcher;

use super::Client;

use anyhow::{anyhow, Result};
use svalin_pki::Certificate;
use svalin_rpc::rpc::connection::{self, Connection};
use tokio::{sync::oneshot, task::JoinSet};

impl Client {
    pub async fn add_agent_with_code(&self, join_code: String) -> Result<WaitingForConfirmCode> {
        let connection = self.rpc.upstream_connection();

        let root = self.root_certificate.clone();
        let upstream = self.upstream_certificate.clone();
        let credentials = self.credentials.clone();

        let (wait_for_confirm_send, wait_for_confirm_recv) = oneshot::channel::<Result<()>>();

        let (confirm_code_send, confirm_code_recv) = oneshot::channel::<String>();

        let (result_send, result_recv) = oneshot::channel::<Result<Certificate>>();

        tokio::spawn(async move {
            let result = connection
                .accept_join(
                    join_code,
                    wait_for_confirm_send,
                    confirm_code_recv,
                    &credentials,
                    &root,
                    &upstream,
                )
                .await;

            result_send.send(result).unwrap();
        });

        let mut join_set = JoinSet::new();

        wait_for_confirm_recv.await??;

        join_set.join_next().await.unwrap()?;

        Ok(WaitingForConfirmCode {
            confirm_code_send,
            result_revc: result_recv,
        })
    }
}

pub struct WaitingForConfirmCode {
    confirm_code_send: oneshot::Sender<String>,
    result_revc: oneshot::Receiver<Result<Certificate>>,
}

impl WaitingForConfirmCode {
    pub async fn confirm(self, confirm_code: String) -> Result<()> {
        self.confirm_code_send.send(confirm_code).unwrap();
        let certificate = self.result_revc.await?;
        println!("got cert: {:?}", certificate);

        Ok(())
    }
}
