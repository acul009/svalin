use async_trait::async_trait;
use svalin_pki::mls::transport_types::MessageToServerTransport;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct UploadMlsHandler(pub mpsc::Sender<MessageToServerTransport>);

#[async_trait]
impl CommandHandler for UploadMlsHandler {
    type Request = ();

    fn key() -> String {
        "upload_mls".to_string()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        while let Some(message) = cancel
            .run_until_cancelled(session.read_object::<MessageToServerTransport>())
            .await
        {
            let message = message?;
            self.0.send(message).await?;
        }
        Ok(())
    }
}

pub struct UploadMls(
    pub mpsc::Receiver<MessageToServerTransport>,
    pub CancellationToken,
);

impl CommandDispatcher for UploadMls {
    type Output = ();

    type Error = anyhow::Error;

    type Request = ();

    fn key() -> String {
        UploadMlsHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        let mut receiver = self.0;
        let cancel = self.1;

        while let Some(message) = cancel.run_until_cancelled(receiver.recv()).await.flatten() {
            session.write_object(&message).await?;
        }
        Ok(())
    }
}
