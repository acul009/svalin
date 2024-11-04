use std::sync::Arc;

use anyhow::anyhow;
use svalin_pki::{
    verifier::{exact::ExactVerififier, VerificationError, Verifier},
    Certificate,
};

use crate::server::{agent_store::AgentStore, user_store::UserStore};

use super::verification_helper::VerificationHelper;

#[derive(Debug)]
pub struct ServerStorageVerifier {
    helper: VerificationHelper,
    agent_verifier: ExactVerififier,
    user_store: Arc<UserStore>,
    agent_store: Arc<AgentStore>,
}

impl ServerStorageVerifier {
    pub fn new(
        helper: VerificationHelper,
        root: Certificate,
        user_store: Arc<UserStore>,
        agent_store: Arc<AgentStore>,
    ) -> Self {
        Self {
            helper,
            agent_verifier: ExactVerififier::new(root),
            user_store,
            agent_store,
        }
    }
}

impl Verifier for ServerStorageVerifier {
    async fn verify_fingerprint(
        &self,
        fingerprint: [u8; 32],
        time: u64,
    ) -> Result<Certificate, svalin_pki::verifier::VerificationError> {
        if let Some(root) = self.helper.try_root(fingerprint) {
            return Ok(root);
        }

        let agent = self.agent_store.get_agent(&fingerprint).await?;
        if let Some(agent) = agent {
            let agent_data = agent.verify(&self.agent_verifier, time).await?;

            return self.helper.help_verify(time, agent_data.unpack().cert);
        }

        let user = self.user_store.get_user(&fingerprint)?;
        if let Some(_user) = user {
            // TODO: make helper actually check certificate chain
            // return self.helper.help_verify(time, _user.certificate);

            return Err(anyhow!("User verification not yet implemented").into());
        }

        Err(VerificationError::UnknownCertificate)
    }
}
