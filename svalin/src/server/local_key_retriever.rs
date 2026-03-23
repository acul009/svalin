use std::sync::Arc;

use anyhow::anyhow;
use svalin_pki::{
    CertificateChainBuilder, RootCertificate, get_current_timestamp,
    mls::key_retriever::KeyRetriever,
};

use crate::server::{
    agent_store::AgentStore, key_package_store::KeyPackageStore, session_store::SessionStore,
    user_store::UserStore,
};

pub struct LocalKeyRetriever {
    root: RootCertificate,
    agent_store: Arc<AgentStore>,
    user_store: Arc<UserStore>,
    session_store: Arc<SessionStore>,
    key_package_store: Arc<KeyPackageStore>,
}

impl LocalKeyRetriever {
    pub fn new(
        root: RootCertificate,
        agent_store: Arc<AgentStore>,
        user_store: Arc<UserStore>,
        session_store: Arc<SessionStore>,
        key_package_store: Arc<KeyPackageStore>,
    ) -> Self {
        Self {
            root,
            agent_store,
            user_store,
            session_store,
            key_package_store,
        }
    }
}

impl KeyRetriever for LocalKeyRetriever {
    type Error = anyhow::Error;

    async fn get_required_device_group_members(
        &self,
        device: &svalin_pki::SpkiHash,
    ) -> Result<Vec<svalin_pki::SpkiHash>, Self::Error> {
        let agent = self
            .agent_store
            .get_agent(&device)
            .await?
            .ok_or_else(|| anyhow!("agent not found"))?;
        let chain = CertificateChainBuilder::new(agent);

        let timestamp = get_current_timestamp();

        let chain = self.user_store.complete_certificate_chain(chain).await?;
        let chain = chain.verify(&self.root, timestamp)?;

        let mut required_members = vec![device.clone()];

        for user_certificate in chain.iter().rev().skip(1) {
            required_members.push(user_certificate.spki_hash().clone());
            let sessions = self
                .session_store
                .list_user_sessions(user_certificate.spki_hash())
                .await?;

            for session in sessions.into_iter() {
                match session.verify_signature(user_certificate, timestamp) {
                    Ok(certificate) => {
                        required_members.push(certificate.spki_hash().clone());
                    }
                    // TODO: report this error somewhere
                    Err(_) => {}
                }
            }
        }

        Ok(required_members)
    }

    async fn get_key_packages(
        &self,
        entities: &[svalin_pki::SpkiHash],
    ) -> Result<Vec<svalin_pki::mls::key_package::UnverifiedKeyPackage>, Self::Error> {
        let key_packages = self
            .key_package_store
            .get_key_packages(entities.iter())
            .await?;

        Ok(key_packages)
    }
}
