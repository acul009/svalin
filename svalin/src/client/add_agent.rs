use crate::shared::join_agent::{
    accept_handler::AcceptJoin,
    add_agent::{UploadAgent, UploadAgentCommandError},
};

use super::Client;

use anyhow::Result;
use svalin_pki::{Certificate, mls::client::DeviceGroupCreationInfo};
use svalin_rpc::rpc::connection::{Connection, ConnectionDispatchError};
use tokio::sync::oneshot;

impl Client {
    pub async fn add_agent_with_code(
        &self,
        join_code: String,
        confirm_code: oneshot::Sender<oneshot::Sender<String>>,
    ) -> Result<Certificate> {
        let connection = self.rpc.upstream_connection();

        let certificate = connection
            .dispatch(AcceptJoin {
                client: &self,
                join_code,
                confirm_code,
            })
            .await?;

        Ok(certificate)
    }

    pub(crate) async fn upload_agent(
        &self,
        group_info: &DeviceGroupCreationInfo,
    ) -> Result<(), ConnectionDispatchError<UploadAgentCommandError>> {
        let connection = self.rpc.upstream_connection();

        connection.dispatch(UploadAgent::new(&group_info)).await?;

        Ok(())
    }
}
