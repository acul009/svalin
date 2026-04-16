use svalin_pki::mls::agent::MlsAgent;
use svalin_rpc::rpc::command::{dispatcher::CommandDispatcher, handler::CommandHandler};
use tokio::sync::{mpsc, oneshot};

use crate::{
    message_streaming::{MessageFromAgent, MessageToAgent, server::AgentMessageHandler},
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
        AgentMessageHandler::key()
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

        Ok(())
    }
}

pub struct AgentMessageReceiver {
    sender: AgentMessageDispatcherHandle,
    mls: MlsAgent<RemoteKeyRetriever, RemoteVerifier>,
}

impl AgentMessageReceiver {
    fn handle(message: MessageToAgent) -> Result<(), anyhow::Error> {
        match message {
            MessageToAgent::Mls(message_to_member_transport) => todo!(),
            MessageToAgent::KeyPackageCount(_) => todo!(),
        }
    }
}
