use crate::shared::join_agent::{accept_handler::AcceptJoin, add_agent::AddAgent, PublicAgentData};

use super::Client;

use anyhow::Result;
use svalin_pki::{signed_object::SignedObject, Certificate, PermCredentials};
use svalin_rpc::rpc::connection::{direct_connection::DirectConnection, Connection};
use tokio::sync::oneshot;
use tracing::debug;

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
            credentials: self.credentials.clone(),
        })
    }
}

pub struct WaitingForConfirmCode {
    connection: DirectConnection,
    confirm_code_send: oneshot::Sender<String>,
    result_revc: oneshot::Receiver<Result<Certificate>>,
    credentials: PermCredentials,
}

impl WaitingForConfirmCode {
    pub async fn confirm(self, confirm_code: String, agent_name: String) -> Result<()> {
        self.confirm_code_send.send(confirm_code).unwrap();
        let certificate = self.result_revc.await??;

        debug!("agent certificate successfully created and sent");

        let agent = SignedObject::new(
            PublicAgentData {
                cert: certificate,
                name: agent_name,
            },
            &self.credentials,
        )?;

        self.connection.dispatch(AddAgent { agent: &agent }).await?;

        debug!("agent is registered on server");

        Ok(())
    }
}
