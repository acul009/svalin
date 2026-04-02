use std::{fmt::Debug, sync::Arc};

use svalin_pki::{CertificateChainBuilder, SpkiHash, UnverifiedCertificateChain};
use svalin_server_store::{AgentStore, SessionStore, UserStore};

#[derive(Debug, Clone)]
pub struct ChainLoader {
    user_store: Arc<UserStore>,
    session_store: Arc<SessionStore>,
    agent_store: Arc<AgentStore>,
}

impl ChainLoader {
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

impl ChainLoader {
    pub async fn load_certificate_chain(
        &self,
        request: &SpkiHash,
    ) -> Result<Option<UnverifiedCertificateChain>, anyhow::Error> {
        let certificate = match self.session_store.get_session(request).await? {
            Some(session) => Some(session),
            None => match self.agent_store.get_agent(request).await? {
                Some(agent) => Some(agent),
                None => match self.user_store.get_user(request).await? {
                    Some(user) => Some(user.encrypted_credential.take_certificate()),
                    None => None,
                },
            },
        };

        let Some(certificate) = certificate else {
            return Ok(None);
        };

        let cert_chain = CertificateChainBuilder::new(certificate);

        let cert_chain = self
            .user_store
            .complete_certificate_chain(cert_chain)
            .await?;

        Ok(Some(cert_chain))
    }
}
