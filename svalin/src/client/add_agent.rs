use std::fmt::Debug;

use crate::shared::join_agent::{
    accept_handler::{AcceptJoin, AcceptJoinError},
    add_agent::AddAgent,
};

use super::Client;

use anyhow::{Result, anyhow};
use svalin_pki::Certificate;
use svalin_rpc::rpc::connection::{
    Connection, ConnectionDispatchError, direct_connection::DirectConnection,
};
use tokio::sync::oneshot;
use tracing::debug;

impl Client {
    pub async fn add_agent_with_code(
        &self,
        join_code: String,
        confirm_code: oneshot::Sender<oneshot::Sender<String>>,
    ) -> Result<()> {
        let connection = self.rpc.upstream_connection();

        let result = connection
            .dispatch(AcceptJoin {
                client: &self,
                join_code,
                confirm_code,
            })
            .await;

        Ok(())
    }
}

// pub struct WaitingForConfirmCode {
//     connection: DirectConnection,
//     confirm_code_send: oneshot::Sender<String>,
//     result_revc: oneshot::Receiver<Result<Certificate, ConnectionDispatchError<AcceptJoinError>>>,
// }

// impl Debug for WaitingForConfirmCode {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("WaitingForConfirmCode").finish()
//     }
// }

// impl WaitingForConfirmCode {
//     pub async fn confirm(self, confirm_code: String) -> Result<()> {
//         self.confirm_code_send.send(confirm_code).unwrap();
//         let certificate = self.result_revc.await?.map_err(|err| anyhow!(err))?;

//         debug!("agent certificate successfully created and sent");

//         self.connection
//             .dispatch(AddAgent::new(&certificate))
//             .await
//             .map_err(|err| anyhow!(err))?;

//         debug!("agent is registered on server");

//         Ok(())
//     }
// }
