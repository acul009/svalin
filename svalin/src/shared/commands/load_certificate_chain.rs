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

pub struct LoadCertificateChainHandler {
    user_store: Arc<UserStore>,
    session_store: Arc<SessionStore>,
    root: Certificate,
}

#[async_trait]
impl CommandHandler for LoadCertificateChainHandler {
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
        let chain_result = self.get_chain(&fingerprint, time).await;

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
    async fn get_chain(
        &self,
        fingerprint: &Fingerprint,
        time: u64,
    ) -> Result<CertificateChain, VerificationError> {
        let certificate = if let Some(user) = self.user_store.get_user(&fingerprint).await? {
            user.encrypted_credential.take_certificate()
        } else if let Some(session) = self.session_store.get_session(&fingerprint).await? {
            session
        } else {
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

pub struct LoadCertificateChain {
    fingerprint: Fingerprint,
}

impl LoadCertificateChain {
    pub fn new(fingerprint: Fingerprint) -> Self {
        Self { fingerprint }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadCertificateChainError {
    #[error("session read error: {0}")]
    SessionReadError(#[from] SessionReadError),
    #[error("session write error: {0}")]
    SessionWriteError(#[from] SessionWriteError),
    #[error("Server was unable to load certificate chain")]
    ServerError,
}

impl CommandDispatcher for LoadCertificateChain {
    type Output = CertificateChain;

    type Error = LoadCertificateChainError;

    type Request = <LoadCertificateChainHandler as CommandHandler>::Request;

    fn key() -> String {
        LoadCertificateChainHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &self.fingerprint
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        let chain_result: Result<CertificateChain, ()> = session.read_object().await?;

        let chain_result = chain_result.map_err(|_| LoadCertificateChainError::ServerError);

        let _ = session.write_object(&()).await;

        chain_result
    }
}
