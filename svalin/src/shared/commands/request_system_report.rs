use std::sync::Arc;

use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

pub struct RequestSystemReportHandler {
    pub notify: Arc<Notify>,
}

#[async_trait]
impl CommandHandler for RequestSystemReportHandler {
    type Request = ();

    fn key() -> String {
        "request-system-report".into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        self.notify.notify_one();

        session.write_object(&()).await?;
        Ok(())
    }
}

pub struct RequestSystemReport;

impl CommandDispatcher for RequestSystemReport {
    type Output = ();
    type Error = anyhow::Error;
    type Request = ();

    fn key() -> String {
        RequestSystemReportHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        session.read_object::<()>().await?;
        Ok(())
    }
}
