use std::sync::Arc;

use svalin_pki::{
    Certificate, CertificateChainBuilder, ExactVerififier, Fingerprint, VerificationError, Verifier,
};

use crate::server::{agent_store::AgentStore, session_store::SessionStore, user_store::UserStore};

#[derive(Debug)]
pub struct IncomingConnectionVerifier {
    root: Certificate,
    agent_verifier: ExactVerififier,
    user_store: Arc<UserStore>,
    session_store: Arc<SessionStore>,
    agent_store: Arc<AgentStore>,
}

impl IncomingConnectionVerifier {
    pub fn new(
        root: Certificate,
        user_store: Arc<UserStore>,
        session_store: Arc<SessionStore>,
        agent_store: Arc<AgentStore>,
    ) -> Self {
        Self {
            // TODO: user verifier
            agent_verifier: ExactVerififier::new(root.clone()),
            root,
            user_store,
            session_store,
            agent_store,
        }
    }
}

impl Verifier for IncomingConnectionVerifier {
    async fn verify_fingerprint(
        &self,
        fingerprint: &Fingerprint,
        time: u64,
    ) -> Result<Certificate, VerificationError> {
        let certificate = if let Some(agent) = self.agent_store.get_agent(fingerprint).await? {
            let agent_data = agent.verify(&self.agent_verifier, time).await?;
            agent_data.unpack().cert
        } else if let Some(session) = self.session_store.get_session(fingerprint).await? {
            session
        } else {
            return Err(VerificationError::UnknownCertificate);
        };

        let cert_chain = CertificateChainBuilder::new(certificate);

        let cert_chain = self
            .user_store
            .complete_certificate_chain(cert_chain)
            .await?;

        let certificate = cert_chain.verify(&self.root, time)?;

        Ok(certificate)
    }
}
