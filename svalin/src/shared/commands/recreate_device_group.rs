use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::agent::MlsAction;

pub struct RecreateDeviceGroupHandler {
    pub actions: mpsc::Sender<MlsAction>,
}

#[async_trait]
impl CommandHandler for RecreateDeviceGroupHandler {
    type Request = ();

    fn key() -> String {
        "recreate-device-group".into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let (send, recv) = oneshot::channel();
        self.actions.send(MlsAction::RecreateGroup(send)).await?;
        let result = recv.await?;
        session.write_object(&result).await?;
        Ok(())
    }
}

pub struct RecreateDeviceGroup;

impl CommandDispatcher for RecreateDeviceGroup {
    type Output = ();
    type Error = anyhow::Error;
    type Request = ();

    fn key() -> String {
        RecreateDeviceGroupHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        session
            .read_object::<Result<(), String>>()
            .await?
            .map_err(anyhow::Error::msg)?;
        Ok(())
    }
}
