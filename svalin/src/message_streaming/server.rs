use std::sync::Arc;

use anyhow::anyhow;
use svalin_pki::{Certificate, mls::transport_types::MessageToServerTransport};
use svalin_server_store::{KeyPackageStore, MessageStore};

use crate::{server::MlsServer, verifier::local_verifier::LocalVerifier};

pub struct MlsMessageHandler {
    pub message_store: Arc<MessageStore>,
    pub key_package_store: Arc<KeyPackageStore>,
    pub mls_server: Arc<MlsServer>,
    pub verifier: LocalVerifier,
}

impl MlsMessageHandler {
    pub async fn handle(
        &self,
        _sender: &Certificate,
        message: MessageToServerTransport,
    ) -> Result<(), anyhow::Error> {
        let to_send = self
            .mls_server
            .process_message(message)
            .await
            .map_err(|err| anyhow!(err))?;
        self.message_store.add_message(to_send).await?;

        Ok(())
    }
}
