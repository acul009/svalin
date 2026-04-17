use std::sync::Arc;

use anyhow::anyhow;
use svalin_client_store::ClientStore;
use svalin_pki::mls::client::MlsClient;
use svalin_rpc::rpc::command::{dispatcher::CommandDispatcher, handler::CommandHandler};
use svalin_sysctl::sytem_report::SystemReport;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{
    client::state::{ClientState, ClientStateUpdate},
    message_streaming::{
        MessageFromClient, MessageToClient,
        with_client::{MessageHandler, MessageSender},
    },
    remote_key_retriever::RemoteKeyRetriever,
    verifier::remote_verifier::RemoteVerifier,
};

#[derive(Clone)]
pub struct ClientMessageDispatcherHandle(
    mpsc::Sender<(MessageFromClient, Option<oneshot::Sender<Result<(), ()>>>)>,
);

impl ClientMessageDispatcherHandle {
    pub async fn send(&self, message: MessageFromClient) {
        let _ = self.0.send((message, None)).await;
    }

    pub async fn try_send(&self, message: MessageFromClient) -> Result<(), ()> {
        let (send, recv) = oneshot::channel();
        self.0.send((message, Some(send))).await.map_err(|_| ())?;
        recv.await.map_err(|_| ())?
    }
}

pub struct ClientMessageDispatcher(
    mpsc::Receiver<(MessageFromClient, Option<oneshot::Sender<Result<(), ()>>>)>,
);

impl ClientMessageDispatcher {
    pub fn new() -> (ClientMessageDispatcherHandle, Self) {
        let (send, recv) = mpsc::channel(100);
        (ClientMessageDispatcherHandle(send), Self(recv))
    }
}

impl CommandDispatcher for ClientMessageDispatcher {
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
        tracing::debug!("Message Dispatcher connected!");
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

pub struct ClientMessageReceiver {
    sender: ClientMessageDispatcherHandle,
    mls: Arc<MlsClient<RemoteKeyRetriever, RemoteVerifier>>,
    cancel: CancellationToken,
    update_sender: mpsc::Sender<ClientStateRequest>,
}

impl ClientMessageReceiver {
    pub async fn initialize(
        sender: ClientMessageDispatcherHandle,
        mls: Arc<MlsClient<RemoteKeyRetriever, RemoteVerifier>>,
        cancel: CancellationToken,
        store: Arc<ClientStore>,
    ) -> Result<(Self, ClientStateHandle), anyhow::Error> {
        let state_handle = ClientStateHandle::initialize(store).await?;

        let me = Self {
            sender,
            mls,
            cancel,
            update_sender: state_handle.channel.clone(),
        };

        Ok((me, state_handle))
    }
}

impl CommandDispatcher for ClientMessageReceiver {
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
        tracing::debug!("client message receiver connected!");
        while let Some(message_result) = self
            .cancel
            .run_until_cancelled(session.read_object::<MessageToClient>())
            .await
        {
            let message = message_result?;

            let handle_result = self.handle(message).await;
            let response = handle_result.as_ref().map(|_| ()).map_err(|_| ());
            session.write_object(&response).await?;

            if let Err(err) = handle_result {
                tracing::error!("Failed to handle message: {err}");
            }
        }

        Ok(())
    }
}

impl ClientMessageReceiver {
    async fn handle(&self, message: MessageToClient) -> Result<(), anyhow::Error> {
        match message {
            MessageToClient::Mls(message) => {
                let _message = self
                    .mls
                    .handle_message::<SystemReport>(message)
                    .await
                    .map_err(|err| anyhow!(err))?;
                todo!();
            }
            MessageToClient::AgentOnlineStatus(spki_hash, online) => {
                self.update_client_state(ClientStateUpdate::AgentOnlineStatus(spki_hash, online))
                    .await;
                Ok(())
            }
        }
    }

    async fn update_client_state(&self, update: ClientStateUpdate) {
        let _ = self
            .update_sender
            .send(ClientStateRequest::Update(update))
            .await;
    }
}

pub struct ClientStateHandle {
    channel: mpsc::Sender<ClientStateRequest>,
}

enum ClientStateRequest {
    Update(ClientStateUpdate),
    Subscribe(oneshot::Sender<(ClientState, broadcast::Receiver<ClientStateUpdate>)>),
}

impl ClientStateHandle {
    async fn initialize(store: Arc<ClientStore>) -> Result<Self, anyhow::Error> {
        let (send, mut recv) = mpsc::channel::<ClientStateRequest>(100);

        let persistent = store.load_persistent().await?;
        let mut state = ClientState::new(persistent);

        tokio::spawn(async move {
            let (update_broadcast, _) = broadcast::channel::<ClientStateUpdate>(100);

            while let Some(request) = recv.recv().await {
                match request {
                    ClientStateRequest::Subscribe(sender) => {
                        let state = state.clone();
                        let receiver = update_broadcast.subscribe();
                        let _ = sender.send((state, receiver));
                    }
                    ClientStateRequest::Update(message) => {
                        if update_broadcast.receiver_count() > 0 {
                            let _ = update_broadcast.send(message.clone());
                        }
                        if let ClientStateUpdate::Persistent(message) = &message {
                            if let Err(err) = store.update(message).await {
                                tracing::error!("Failed to update persistent state: {}", err);
                            }
                        }
                        state.update(message);
                    }
                }
            }
        });

        Ok(Self { channel: send })
    }

    pub async fn subscribe(
        &self,
    ) -> Result<(ClientState, broadcast::Receiver<ClientStateUpdate>), anyhow::Error> {
        let (sender, receiver) = oneshot::channel();
        self.channel
            .send(ClientStateRequest::Subscribe(sender))
            .await?;
        Ok(receiver.await?)
    }
}
