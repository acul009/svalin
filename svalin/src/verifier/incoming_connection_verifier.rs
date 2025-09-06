use std::sync::Arc;

use svalin_pki::{Certificate, ExactVerififier, Fingerprint, VerificationError, Verifier};

use crate::server::{agent_store::AgentStore, session_store::SessionStore, user_store::UserStore};

use super::verification_helper::VerificationHelper;

#[derive(Debug)]
pub struct IncomingConnectionVerifier {
    helper: VerificationHelper,
    agent_verifier: ExactVerififier,
    user_store: Arc<UserStore>,
    session_store: Arc<SessionStore>,
    agent_store: Arc<AgentStore>,
}

impl IncomingConnectionVerifier {
    pub fn new(
        helper: VerificationHelper,
        root: Certificate,
        user_store: Arc<UserStore>,
        session_store: Arc<SessionStore>,
        agent_store: Arc<AgentStore>,
    ) -> Self {
        Self {
            helper,
            agent_verifier: ExactVerififier::new(root),
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
        if let Some(root) = self.helper.try_root(fingerprint) {
            return Ok(root);
        }

        let agent = self.agent_store.get_agent(fingerprint).await?;
        if let Some(agent) = agent {
            let agent_data = agent.verify(&self.agent_verifier, time).await?;

            return self
                .helper
                .help_verify(time, agent_data.unpack().cert)
                .await;
        }

        let session = self.session_store.get_session(fingerprint).await?;
        if let Some(session) = session {
            // TODO: make helper actually check certificate chain
            return self.helper.help_verify(time, session).await;
        }

        Err(VerificationError::UnknownCertificate)
    }
}
