use std::sync::Arc;

use anyhow::anyhow;
use svalin_pki::mls::agent::MlsAgent;
use svalin_rpc::rpc::command::{dispatcher::CommandDispatcher, handler::CommandHandler};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{
    message_streaming::{
        MessageFromAgent, MessageToAgent,
        with_agent::{MessageHandler, MessageSender},
    },
    remote_key_retriever::RemoteKeyRetriever,
    verifier::remote_verifier::RemoteVerifier,
};

#[derive(Clone)]
pub struct AgentMessageDispatcherHandle(
    mpsc::Sender<(MessageFromAgent, Option<oneshot::Sender<Result<(), ()>>>)>,
);

impl AgentMessageDispatcherHandle {
    pub async fn send(&self, message: MessageFromAgent) {
        let _ = self.0.send((message, None)).await;
    }

    pub async fn try_send(&self, message: MessageFromAgent) -> Result<(), ()> {
        let (send, recv) = oneshot::channel();
        self.0.send((message, Some(send))).await.map_err(|_| ())?;
        recv.await.map_err(|_| ())?
    }
}

pub struct AgentMessageDispatcher(
    mpsc::Receiver<(MessageFromAgent, Option<oneshot::Sender<Result<(), ()>>>)>,
);

impl AgentMessageDispatcher {
    pub fn new() -> (AgentMessageDispatcherHandle, Self) {
        let (send, recv) = mpsc::channel(100);
        (AgentMessageDispatcherHandle(send), Self(recv))
    }
}

impl CommandDispatcher for AgentMessageDispatcher {
    type Output = ();

    type Error = anyhow::Error;

    type Request = ();

    fn key() -> String {
        MessageHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(
        mut self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        while let Some((message, feedback)) = self.0.recv().await {
            session.write_object(&message).await?;

            let result = session.read_object::<Result<(), ()>>().await?;
            if let Some(feedback) = feedback {
                let _ = feedback.send(result);
            }
        }

        session.write_object(&MessageFromAgent::Goodbye).await?;
        let _result = session.read_object::<Result<(), ()>>().await?;

        Ok(())
    }
}

pub struct AgentMessageReceiver {
    pub sender: AgentMessageDispatcherHandle,
    pub mls: Arc<MlsAgent<RemoteKeyRetriever, RemoteVerifier>>,
    pub cancel: CancellationToken,
}

impl CommandDispatcher for AgentMessageReceiver {
    type Output = ();

    type Error = anyhow::Error;

    type Request = ();

    fn key() -> String {
        MessageSender::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(
        self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        while let Some(message_result) = self
            .cancel
            .run_until_cancelled(session.read_object::<MessageToAgent>())
            .await
        {
            let message = message_result?;

            let handle_result = self.handle(message).await;
            let shutdown = handle_result.as_ref().cloned().unwrap_or(false);
            let response = handle_result.as_ref().map(|_| ()).map_err(|_| ());
            session.write_object(&response).await?;
            if shutdown {
                break;
            }

            if let Err(err) = handle_result {
                tracing::error!("Failed to handle message: {err}");
            }
        }

        Ok(())
    }
}

impl AgentMessageReceiver {
    async fn handle(&self, message: MessageToAgent) -> Result<bool, anyhow::Error> {
        match message {
            MessageToAgent::Mls(message) => {
                self.mls
                    .handle_message(message)
                    .await
                    .map_err(|err| anyhow!(err))?;
                todo!();
            }
            MessageToAgent::Goodbye => Ok(true),
        }
    }
}
