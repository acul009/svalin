use anyhow::Context;
use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use tokio::select;
use tokio_util::sync::CancellationToken;

pub struct UpdateAgentHandler {
    mutex: tokio::sync::Mutex<()>,
}

impl UpdateAgentHandler {
    pub fn new() -> Self {
        Self {
            mutex: tokio::sync::Mutex::new(()),
        }
    }
}

#[async_trait]
impl CommandHandler for UpdateAgentHandler {
    type Request = String;

    fn key() -> String {
        "update-agent".into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let Ok(_guard) = self.mutex.try_lock() else {
            let err = Result::<(), String>::Err("update already in progress".into());
            session.write_object(&err).await?;
            return Ok(());
        };

        select! {
            update_result = crate::installer::update_agent(&request) => {
                let update_result = update_result.context("error while executing update");
                let send_result: Result<(), String> = match &update_result {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e.to_string()),
                };
                session.write_object(&send_result).await?;
                update_result
            }
            _ = cancel.cancelled() => {
                let send_result: Result<(), String> = Ok(());
                session.write_object(&send_result).await?;
                Ok(())
            }
        }
    }
}

pub struct UpdateAgent(pub String);

impl CommandDispatcher for UpdateAgent {
    type Output = ();

    type Error = anyhow::Error;

    type Request = String;

    fn key() -> String {
        UpdateAgentHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &self.0
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        let result = session.read_object::<Result<(), String>>().await?;

        result.map_err(|e| anyhow::anyhow!(e))
    }
}
