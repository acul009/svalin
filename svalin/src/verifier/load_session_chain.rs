use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use svalin_pki::{
    Certificate, CertificateChain, CertificateChainBuilder, Fingerprint, VerificationError,
};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError, SessionWriteError},
};
use tokio_util::sync::CancellationToken;

use crate::server::{session_store::SessionStore, user_store::UserStore};

pub struct LoadSessionChainHandler {
    user_store: Arc<UserStore>,
    session_store: Arc<SessionStore>,
    root: Certificate,
}

impl LoadSessionChainHandler {
    pub fn new(
        user_store: Arc<UserStore>,
        session_store: Arc<SessionStore>,
        root: Certificate,
    ) -> Self {
        Self {
            user_store,
            session_store,
            root,
        }
    }
}

#[async_trait]
impl CommandHandler for LoadSessionChainHandler {
    type Request = Fingerprint;

    fn key() -> String {
        "load_certificate_chain".to_string()
    }

    async fn handle(
        &self,
        session: &mut Session,
        fingerprint: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let chain_result = self.get_session_chain(&fingerprint, time).await;

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

impl LoadSessionChainHandler {
    async fn get_session_chain(
        &self,
        fingerprint: &Fingerprint,
        time: u64,
    ) -> Result<CertificateChain, VerificationError> {
        let Some(certificate) = self.session_store.get_session(&fingerprint).await? else {
            return Err(VerificationError::UnknownCertificate);
        };

        let cert_chain = CertificateChainBuilder::new(certificate);

        let cert_chain = self
            .user_store
            .complete_certificate_chain(cert_chain)
            .await?;

        cert_chain.verify(&self.root, time)?;

        Ok(cert_chain)
    }
}

pub struct LoadSessionChain<'a> {
    pub fingerprint: &'a Fingerprint,
}

impl<'a> LoadSessionChain<'a> {
    pub fn new(fingerprint: &'a Fingerprint) -> Self {
        Self { fingerprint }
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

impl CommandDispatcher for LoadSessionChain<'_> {
    type Output = CertificateChain;

    type Error = LoadSessionChainError;

    type Request = <LoadSessionChainHandler as CommandHandler>::Request;

    fn key() -> String {
        LoadSessionChainHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &self.fingerprint
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        let chain_result: Result<CertificateChain, ()> = session.read_object().await?;

        let chain_result = chain_result.map_err(|_| LoadSessionChainError::ServerError);

        let _ = session.write_object(&()).await;

        chain_result
    }
}
