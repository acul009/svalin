use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use dashmap::DashMap;
use svalin_pki::{Certificate, CertificateType, SpkiHash};
use svalin_rpc::rpc::{
    command::handler::CommandHandler, peer::Peer, server::RpcServer, session::Session,
};
use tokio::sync::{broadcast, mpsc, oneshot};
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
    ) -> Result<(), anyhow::Error> {
        match message {
            MessageFromClient::Mls(mls) => self.mls_handler.handle(session, mls).await,
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
            let response = handle_result.as_ref().map(|_| ()).map_err(|_| ());
            session.write_object(&response).await?;

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

        // stream agent online status
        let server = self.server.clone();
        let sender2 = sender.clone();
        tokio::spawn(async move {
            // TODO: access permissions
            let sender = sender2;
            let mut recv = server.subscribe_to_connection_status();
            for connected in server.get_current_connected_clients().await {
                let _ = sender
                    .send((MessageToClient::AgentOnlineStatus(connected, true), None))
                    .await;
            }
            while let Ok((spki_hash, online)) = recv.recv().await {
                tracing::debug!("peer online status: {spki_hash:?} {online}");
                let _ = sender
                    .send((MessageToClient::AgentOnlineStatus(spki_hash, online), None))
                    .await;
            }
        });

        let spki_hash = peer.spki_hash().clone();
        self.channels.insert(spki_hash.clone(), sender);

        let result = self.handle_connection(session, receiver).await;

        self.channels.remove(&spki_hash);

        result
    }
}

impl MessageSender {
    pub fn new(server: Arc<RpcServer>) -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
            server,
        }
    }

    async fn handle_connection(
        &self,
        session: &mut Session,
        mut receiver: mpsc::Receiver<(MessageToClient, Option<oneshot::Sender<Result<(), ()>>>)>,
    ) -> Result<(), anyhow::Error> {
        while let Some((message, feedback)) = receiver.recv().await {
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
