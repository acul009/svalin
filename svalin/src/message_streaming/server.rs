use std::sync::Arc;

use crate::store::server_store::{KeyPackageStore, MessageStore};
use anyhow::anyhow;
use svalin_pki::{
    Certificate, CertificateType, TrustStoreVerifier,
    mls::transport_types::MessageToServerTransport,
};

pub struct MlsMessageHandler {
    pub message_store: Arc<MessageStore>,
    pub key_package_store: Arc<KeyPackageStore>,
    pub mls_server: Arc<crate::server::MlsServer>,
    pub verifier: TrustStoreVerifier,
}

impl MlsMessageHandler {
    pub async fn handle(
        &self,
        sender: &Certificate,
        message: MessageToServerTransport,
    ) -> Result<(), anyhow::Error> {
        let group_to_clear = match &message {
            MessageToServerTransport::NewDeviceGroup(group) => Some(group.group_id()?),
            _ => None,
        };

        let messages_to_send = self
            .mls_server
            .process_message(message)
            .await
            .map_err(|err| anyhow!(err))?;

        if let Some(group_id) = group_to_clear {
            self.message_store
                .clear_group_messages(group_id.as_slice())
                .await?;
        }

        for mut to_send in messages_to_send {
            match sender.certificate_type() {
                CertificateType::UserSession => {
                    to_send.remove_receiver(sender.issuer());
                }
                CertificateType::Agent => {
                    to_send.remove_receiver(sender.spki_hash());
                }
                _ => {
                    return Err(anyhow!(
                        "Certificate type is not supposed to send messages yet"
                    ));
                }
            }
            self.message_store.add_message(to_send).await?;
        }

        Ok(())
    }
}
