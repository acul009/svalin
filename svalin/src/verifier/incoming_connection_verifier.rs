use std::sync::Arc;

use svalin_pki::{
    Certificate, CertificateChainBuilder, RootCertificate, SpkiHash, Verifier, VerifyError,
};

use crate::server::{agent_store::AgentStore, session_store::SessionStore, user_store::UserStore};

#[derive(Debug, Clone)]
pub struct IncomingConnectionVerifier {
    root: RootCertificate,
    user_store: Arc<UserStore>,
    session_store: Arc<SessionStore>,
    agent_store: Arc<AgentStore>,
}

impl IncomingConnectionVerifier {
    pub fn new(
        root: RootCertificate,
        user_store: Arc<UserStore>,
        session_store: Arc<SessionStore>,
        agent_store: Arc<AgentStore>,
    ) -> Self {
        Self {
            // TODO: user verifier
            root,
            user_store,
            session_store,
            agent_store,
        }
    }
}

impl Verifier for IncomingConnectionVerifier {
    async fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> Result<Certificate, VerifyError> {
        let certificate = if let Some(agent) = self.agent_store.get_agent(spki_hash).await? {
            agent
        } else if let Some(session) = self.session_store.get_session(spki_hash).await? {
            session
        } else {
            return Err(VerifyError::UnknownCertificate);
        };

        let cert_chain = CertificateChainBuilder::new(certificate);

        let cert_chain = self
            .user_store
            .complete_certificate_chain(cert_chain)
            .await?;

        let cert_chain = cert_chain.verify(&self.root, time)?;

        Ok(cert_chain.take_leaf())
    }
}
