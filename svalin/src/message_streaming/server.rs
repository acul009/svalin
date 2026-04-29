use std::sync::Arc;

use anyhow::anyhow;
use svalin_pki::{Certificate, mls::transport_types::MessageToServerTransport};
use svalin_server_store::{KeyPackageStore, MessageStore};

use crate::verifier::local_verifier::LocalVerifier;

pub struct MlsMessageHandler {
    pub message_store: Arc<MessageStore>,
    pub key_package_store: Arc<KeyPackageStore>,
    pub mls_server: Arc<crate::server::MlsServer>,
    pub verifier: LocalVerifier,
}

impl MlsMessageHandler {
    pub async fn handle(
        &self,
        sender: &Certificate,
        message: MessageToServerTransport,
    ) -> Result<(), anyhow::Error> {
        let messages_to_send = self
            .mls_server
            .process_message(message)
            .await
            .map_err(|err| anyhow!(err))?;
        for mut to_send in messages_to_send {
            to_send.receivers = to_send
                .receivers
                .into_iter()
                .filter(|target| target != sender.spki_hash())
                .collect();
            self.message_store.add_message(to_send).await?;
        }

        Ok(())
    }
}
