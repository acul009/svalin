use crate::{message_streaming::MessageFromClient, shared::join_agent::accept_handler::AcceptJoin};

use super::Client;

use anyhow::Result;
use svalin_pki::{Certificate, trust_store};
use svalin_rpc::rpc::connection::Connection;
use svalin_store::trust_store_transaction_store::TransactionStoreError;
use tokio::sync::oneshot;

impl Client {
    pub async fn add_agent_with_code(
        &self,
        join_code: String,
        confirm_code: oneshot::Sender<oneshot::Sender<String>>,
    ) -> Result<Certificate> {
        let connection = self.rpc.upstream_connection();

        let certificate = connection
            .dispatch(AcceptJoin {
                client: &self,
                join_code,
                confirm_code,
            })
            .await?;

        Ok(certificate)
    }

    pub(crate) async fn add_cert_to_trust_store(
        &self,
        cert: Certificate,
    ) -> Result<(), AddToTrustStoreError> {
        let block = self
            .trust_store
            .write()
            .unwrap()
            .add(cert, &self.user_credential)?;
        self.message_sender
            .send_with_feedback(MessageFromClient::TrustStore(block.as_unchecked().clone()))
            .await
            .map_err(|_| AddToTrustStoreError::UploadToServerError)?;
        self.store.transaction_store().add(&block).await?;
        self.trust_store.write().unwrap().apply(block);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AddToTrustStoreError {
    #[error("error creating block: {0}")]
    CreateBlockError(#[from] trust_store::CreateBlockError),
    #[error("error while sending block to server")]
    UploadToServerError,
    #[error("error saving transaction")]
    TransactionStoreError(#[from] TransactionStoreError),
}
