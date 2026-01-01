use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{CertificateChainBuilder, SpkiHash, UnverifiedCertificateChain};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError, SessionWriteError},
};
use tokio_util::sync::CancellationToken;

use crate::server::{agent_store::AgentStore, session_store::SessionStore, user_store::UserStore};

pub struct LoadCertificateChainHandler {
    user_store: Arc<UserStore>,
    session_store: Arc<SessionStore>,
    agent_store: Arc<AgentStore>,
}

impl LoadCertificateChainHandler {
    pub fn new(
        user_store: Arc<UserStore>,
        agent_store: Arc<AgentStore>,
        session_store: Arc<SessionStore>,
    ) -> Self {
        Self {
            user_store,
            session_store,
            agent_store,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum ChainRequest {
    Session(SpkiHash),
    Agent(SpkiHash),
}

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
        let chain_result = self.get_certificate_chain(&request).await;

        let (chain, result) = match chain_result {
            Ok(chain) => (Some(chain), Ok(())),
            Err(e) => (None, Err(e)),
        };

        let chain = chain.ok_or(());
        let _ = session.write_object(&chain).await;

        let _answer = session.read_object::<()>().await;

        result?;

        Ok(())
    }
}

impl LoadCertificateChainHandler {
    async fn get_certificate_chain(
        &self,
        request: &ChainRequest,
    ) -> Result<UnverifiedCertificateChain, anyhow::Error> {
        let certificate = match request {
            ChainRequest::Session(spki_hash) => self.session_store.get_session(spki_hash).await?,
            ChainRequest::Agent(spki_hash) => self.agent_store.get_agent(spki_hash).await?,
        };

        let Some(certificate) = certificate else {
            return Err(anyhow::anyhow!("Certificate not found"));
        };

        let cert_chain = CertificateChainBuilder::new(certificate);

        let cert_chain = self
            .user_store
            .complete_certificate_chain(cert_chain)
            .await?;

        Ok(cert_chain)
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
