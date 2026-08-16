use std::{fmt::Debug, sync::Arc};

use svalin_pki::{
    CertificateChainBuilder, SpkiHash, UnverifiedCertificateChain,
    trust_store::{self, TrustStore},
};
use svalin_server_store::{AgentStore, SessionStore, UserStore};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct ChainLoader {
    trust_store: Arc<RwLock<TrustStore>>,
    user_store: Arc<UserStore>,
    session_store: Arc<SessionStore>,
}

impl ChainLoader {
    pub fn new(
        trust_store: Arc<RwLock<TrustStore>>,
        user_store: Arc<UserStore>,
        session_store: Arc<SessionStore>,
    ) -> Self {
        Self {
            trust_store,
            user_store,
            session_store,
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
