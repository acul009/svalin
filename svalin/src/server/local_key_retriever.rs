use std::sync::Arc;

use anyhow::anyhow;
use svalin_pki::{
    CertificateChainBuilder, RootCertificate, get_current_timestamp,
    mls::{SvalinGroupId, key_retriever::KeyRetriever},
};
use svalin_server_store::{AgentStore, KeyPackageStore, UserStore};

pub struct LocalKeyRetriever {
    root: RootCertificate,
    agent_store: Arc<AgentStore>,
    user_store: Arc<UserStore>,
    key_package_store: Arc<KeyPackageStore>,
}

impl LocalKeyRetriever {
    pub fn new(
        root: RootCertificate,
        agent_store: Arc<AgentStore>,
        user_store: Arc<UserStore>,
        key_package_store: Arc<KeyPackageStore>,
    ) -> Self {
        Self {
            root,
            agent_store,
            user_store,
            key_package_store,
        }
    }
}

impl KeyRetriever for LocalKeyRetriever {
    type Error = anyhow::Error;

    async fn get_required_group_members(
        &self,
        id: &SvalinGroupId,
    ) -> Result<Vec<svalin_pki::SpkiHash>, Self::Error> {
        match id {
            SvalinGroupId::DeviceGroup(spki_hash) => {
                let agent = self
                    .agent_store
                    .get_agent(&spki_hash)
                    .await?
                    .ok_or_else(|| anyhow!("agent not found"))?;
                let chain = CertificateChainBuilder::new(agent);

                let timestamp = get_current_timestamp();

                let chain = self.user_store.complete_certificate_chain(chain).await?;
                let chain = chain.verify(&self.root, timestamp)?;

                let required_members = chain.iter().map(|cert| cert.spki_hash().clone()).collect();

                Ok(required_members)
            }
        }
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
