use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{SpkiHash, UnverifiedCertificateChain};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError, SessionWriteError},
};
use tokio_util::sync::CancellationToken;

use crate::server::chain_loader::ChainLoader;

pub struct LoadCertificateChainHandler {
    loader: ChainLoader,
}

impl LoadCertificateChainHandler {
    pub fn new(loader: ChainLoader) -> Self {
        Self { loader }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ChainRequest(pub SpkiHash);

#[async_trait]
impl CommandHandler for LoadCertificateChainHandler {
    type Request = ChainRequest;

    fn key() -> String {
        "load_certificate_chain".to_string()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let chain_result = self.loader.load_certificate_chain(&request.0).await;

        let (chain, result) = match chain_result {
            Ok(Some(chain)) => (Some(chain), Ok(())),
            Ok(None) => (None, Err(anyhow!("Certificate not found"))),
            Err(e) => (None, Err(e)),
        };

        let chain = chain.ok_or(());
        let _ = session.write_object(&chain).await;

        let _answer = session.read_object::<()>().await;

        result?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadSessionChainError {
    #[error("session read error: {0}")]
    SessionReadError(#[from] SessionReadError),
    #[error("session write error: {0}")]
    SessionWriteError(#[from] SessionWriteError),
    #[error("Server was unable to load certificate chain")]
    ServerError,
}

impl CommandDispatcher for ChainRequest {
    type Output = UnverifiedCertificateChain;

    type Error = LoadSessionChainError;

    type Request = Self;

    fn key() -> String {
        LoadCertificateChainHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        self
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        let chain_result: Result<UnverifiedCertificateChain, ()> = session.read_object().await?;

        let chain_result = chain_result.map_err(|_| LoadSessionChainError::ServerError);

        let _ = session.write_object(&()).await;

        chain_result
    }
}
