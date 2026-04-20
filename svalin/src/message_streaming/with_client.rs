use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use dashmap::DashMap;
use svalin_pki::{Certificate, CertificateType, SpkiHash};
use svalin_rpc::rpc::{
    command::handler::CommandHandler, peer::Peer, server::RpcServer, session::Session,
};
use svalin_server_store::MessageStore;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::message_streaming::{MessageFromClient, MessageToClient, server::MlsMessageHandler};

pub struct MessageHandler {
    pub mls_handler: Arc<MlsMessageHandler>,
}

impl MessageHandler {
    async fn handle(
        &self,
        session: &Certificate,
        message: MessageFromClient,
    ) -> Result<bool, anyhow::Error> {
        match message {
            MessageFromClient::Mls(mls) => {
                self.mls_handler.handle(session, mls).await.map(|_| false)
            }
            MessageFromClient::Goodbye => Ok(true),
        }
    }
}

#[async_trait]
impl CommandHandler for MessageHandler {
    type Request = ();

    fn key() -> String {
        "message-sending-client".into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let Peer::Certificate(peer) = session.peer() else {
            return Err(anyhow!("Expected peer to be a certificate"));
        };
        if peer.certificate_type() != CertificateType::UserSession {
            return Err(anyhow!("Expected peer to be a session"));
        }
        let peer = peer.clone();

        while let Some(message_result) = cancel
            .run_until_cancelled(session.read_object::<MessageFromClient>())
            .await
        {
            let message = message_result?;

            let handle_result = self.handle(&peer, message).await;
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

#[derive(Clone)]
pub struct MessageSender {
    channels: Arc<
        DashMap<SpkiHash, mpsc::Sender<(MessageToClient, Option<oneshot::Sender<Result<(), ()>>>)>>,
    >,
    server: Arc<RpcServer>,
    message_store: Arc<MessageStore>,
}

#[async_trait]
impl CommandHandler for MessageSender {
    type Request = ();

    fn key() -> String {
        "message-receiving-client".into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let Peer::Certificate(peer) = session.peer() else {
            return Err(anyhow!("Expected peer to be a certificate"));
        };
        if peer.certificate_type() != CertificateType::UserSession {
            return Err(anyhow!("Expected peer to be a session"));
        };

        tracing::debug!("client {peer:?} now receiving messages");

        let (sender, receiver) = mpsc::channel(10);

        tokio::spawn(stream_agent_online_status(
            sender.clone(),
            self.server.clone(),
        ));

        tokio::spawn(stream_mls_messages(
            peer.spki_hash().clone(),
            sender.clone(),
            self.message_store.clone(),
        ));

        let spki_hash = peer.spki_hash().clone();
        self.channels.insert(spki_hash.clone(), sender);

        let result = self.handle_connection(session, receiver).await;

        self.channels.remove(&spki_hash);

        result
    }
}

impl MessageSender {
    pub fn new(server: Arc<RpcServer>, message_store: Arc<MessageStore>) -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
            server,
            message_store,
        }
    }

    async fn handle_connection(
        &self,
        session: &mut Session,
        mut receiver: mpsc::Receiver<(MessageToClient, Option<oneshot::Sender<Result<(), ()>>>)>,
    ) -> Result<(), anyhow::Error> {
        while let Some((message, feedback)) = receiver.recv().await {
            tracing::debug!("Sending message to client: {:?}", message);
            session.write_object(&message).await?;

            let response = session.read_object::<Result<(), ()>>().await?;
            if let Some(feedback) = feedback {
                let _ = feedback.send(response);
            }
        }

        Ok(())
    }

    pub async fn send_message(&self, spki_hash: &SpkiHash, message: MessageToClient) {
        let sender = {
            let Some(sender) = self.channels.get(spki_hash) else {
                return;
            };
            sender.clone()
        };

        let _ = sender.send((message, None)).await;
    }

    pub async fn try_send_message(
        &self,
        spki_hash: &SpkiHash,
        message: MessageToClient,
    ) -> Result<(), anyhow::Error> {
        let sender = {
            let Some(sender) = self.channels.get(spki_hash) else {
                return Err(anyhow!("No channel for session"));
            };
            sender.clone()
        };

        let (feedback, receiver) = oneshot::channel();
        sender.send((message, Some(feedback))).await?;

        receiver
            .await?
            .map_err(|_| anyhow!("client returned error for message"))?;

        Ok(())
    }

    pub fn get_sender(
        &self,
        spki_hash: &SpkiHash,
    ) -> Option<mpsc::Sender<(MessageToClient, Option<oneshot::Sender<Result<(), ()>>>)>> {
        self.channels.get(spki_hash).map(|sender| sender.clone())
    }
}

async fn stream_agent_online_status(
    sender: mpsc::Sender<(MessageToClient, Option<oneshot::Sender<Result<(), ()>>>)>,
    server: Arc<RpcServer>,
) {
    // TODO: access permissions
    let mut recv = server.subscribe_to_connection_status();
    for connected in server.get_current_connected_clients().await {
        if connected.certificate_type() != CertificateType::Agent {
            continue;
        }
        let _ = sender
            .send((
                MessageToClient::AgentOnlineStatus(connected.spki_hash().clone(), true),
                None,
            ))
            .await;
    }
    while let Ok((cert, online)) = recv.recv().await {
        if cert.certificate_type() != CertificateType::Agent {
            continue;
        }
        let _ = sender
            .send((
                MessageToClient::AgentOnlineStatus(cert.spki_hash().clone(), online),
                None,
            ))
            .await;
    }
}

async fn stream_mls_messages(
    receiver: SpkiHash,
    sender: mpsc::Sender<(MessageToClient, Option<oneshot::Sender<Result<(), ()>>>)>,
    message_store: Arc<MessageStore>,
) {
    if let Err(e) = stream_mls_messages_inner(receiver, sender, message_store).await {
        tracing::error!("Error streaming mls messages: {}", e);
    }
}

async fn stream_mls_messages_inner(
    receiver: SpkiHash,
    sender: mpsc::Sender<(MessageToClient, Option<oneshot::Sender<Result<(), ()>>>)>,
    message_store: Arc<MessageStore>,
) -> Result<(), anyhow::Error> {
    let current = message_store.load_all_for(&receiver).await?;
    let mut subscription = message_store.subscribe(receiver.clone()).await;
    for message in current {
        let (send, recv) = oneshot::channel();

        let _ = sender
            .send((MessageToClient::Mls(Arc::new(message.1)), Some(send)))
            .await;

        recv.await?
            .map_err(|_| anyhow!("client encountered error with message"))?;

        message_store
            .aknowledge_single_message(&receiver, message.0)
            .await?;
    }

    while let Some(message) = subscription.recv().await {
        tracing::debug!("received message from subscription: {:?}", message);
        let (send, recv) = oneshot::channel();

        let _ = sender
            .send((MessageToClient::Mls(message.1), Some(send)))
            .await;

        recv.await?
            .map_err(|_| anyhow!("client encountered error with message"))?;

        message_store
            .aknowledge_single_message(&receiver, message.0)
            .await?;
    }

    Ok(())
}
