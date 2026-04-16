use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use dashmap::DashMap;
use svalin_pki::{Certificate, CertificateType, SpkiHash};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    peer::Peer,
    session::Session,
};
use svalin_server_store::{KeyPackageStore, MessageStore};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{
    message_streaming::{MessageFromAgent, MessageToAgent, MlsToServer},
    server::MlsServer,
    verifier::local_verifier::LocalVerifier,
};

pub struct AgentMessageHandler {
    mls_handler: Arc<MlsMessageHandler>,
}

impl AgentMessageHandler {
    async fn handle(
        &self,
        agent: &Certificate,
        message: MessageFromAgent,
    ) -> Result<(), anyhow::Error> {
        match message {
            MessageFromAgent::Mls(mls) => self.mls_handler.handle(agent, mls).await,
        }
    }
}

pub struct MlsMessageHandler {
    message_store: Arc<MessageStore>,
    key_package_store: Arc<KeyPackageStore>,
    mls_server: Arc<MlsServer>,
    verifier: LocalVerifier,
}

impl MlsMessageHandler {
    async fn handle(
        &self,
        sender: &Certificate,
        message: MlsToServer,
    ) -> Result<(), anyhow::Error> {
        match message {
            MlsToServer::Mls(message) => {
                let to_send = self
                    .mls_server
                    .process_message(message)
                    .await
                    .map_err(|err| anyhow!(err))?;
                self.message_store.add_message(to_send).await?;

                Ok(())
            }
            MlsToServer::KeyPackage(key_package) => {
                let key_package = self
                    .mls_server
                    .verify_key_package(key_package, &self.verifier)
                    .await?;

                self.key_package_store.add_key_package(key_package).await?;

                Ok(())
            }
        }
    }
}

#[async_trait]
impl CommandHandler for AgentMessageHandler {
    type Request = ();

    fn key() -> String {
        "message-sending-agent".into()
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
        if peer.certificate_type() != CertificateType::Agent {
            return Err(anyhow!("Expected peer to be an agent"));
        }
        let peer = peer.clone();

        loop {
            let Some(message_result) = cancel
                .run_until_cancelled(session.read_object::<MessageFromAgent>())
                .await
            else {
                return Ok(());
            };

            let message = message_result?;

            let handle_result = self.handle(&peer, message).await;
            let response = handle_result.as_ref().map(|_| ()).map_err(|_| ());
            session.write_object(&response).await?;

            if let Err(err) = handle_result {
                tracing::error!("Failed to handle message: {err}");
            }
        }
    }
}

pub struct AgentMessageSender {
    channels:
        DashMap<SpkiHash, mpsc::Sender<(MessageToAgent, Option<oneshot::Sender<Result<(), ()>>>)>>,
}

#[async_trait]
impl CommandHandler for AgentMessageSender {
    type Request = ();

    fn key() -> String {
        "message-receiving-agent".into()
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
        if peer.certificate_type() != CertificateType::Agent {
            return Err(anyhow!("Expected peer to be an agent"));
        };

        let (sender, mut receiver) = mpsc::channel(10);
        self.channels.insert(peer.spki_hash().clone(), sender);

        while let Some((message, feedback)) = receiver.recv().await {
            session.write_object(&message).await?;

            let response = session.read_object::<Result<(), ()>>().await?;
            if let Some(feedback) = feedback {
                let _ = feedback.send(response);
            }
        }

        Ok(())
    }
}
