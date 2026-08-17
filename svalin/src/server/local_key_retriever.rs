use std::sync::{Arc, RwLock};

use anyhow::anyhow;
use svalin_pki::{
    CertificateChainBuilder, RootCertificate, get_current_timestamp,
    mls::{SvalinGroupId, key_retriever::KeyRetriever},
    trust_store::TrustStore,
};
use svalin_server_store::KeyPackageStore;

pub struct LocalKeyRetriever {
    root: RootCertificate,
    trust_store: Arc<RwLock<TrustStore>>,
    key_package_store: Arc<KeyPackageStore>,
}

impl LocalKeyRetriever {
    pub fn new(
        root: RootCertificate,
        trust_store: Arc<RwLock<TrustStore>>,
        key_package_store: Arc<KeyPackageStore>,
    ) -> Self {
        Self {
            root,
            trust_store,
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
                let trust_store = self.trust_store.read().unwrap();
                let agent = trust_store
                    .get(&spki_hash)
                    .ok_or_else(|| anyhow!("agent not found"))?
                    .clone();
                let chain = CertificateChainBuilder::new(agent.to_unverified());

                let timestamp = get_current_timestamp();

                let chain = trust_store.complete_certificate_chain(chain)?;
                let chain = chain.verify(&self.root, timestamp)?;

                let required_members = chain.iter().map(|cert| cert.spki_hash().clone()).collect();

                Ok(required_members)
            }
            SvalinGroupId::DeviceMetaGroup(spki_hash) => {
                let trust_store = self.trust_store.read().unwrap();
                let agent = trust_store
                    .get(&spki_hash)
                    .ok_or_else(|| anyhow!("agent not found"))?
                    .clone();
                let chain = CertificateChainBuilder::new(agent.to_unverified());

                let timestamp = get_current_timestamp();

                let chain = trust_store.complete_certificate_chain(chain)?;
                let chain = chain.verify(&self.root, timestamp)?;

                let required_members = chain
                    .iter()
                    // Skip the device itself
                    .take(chain.iter().len() - 1)
                    .map(|cert| {
                        tracing::trace!(
                            "server required meta member: {:?}",
                            cert.certificate_type()
                        );
                        cert
                    })
                    .map(|cert| cert.spki_hash().clone())
                    .collect();

                Ok(required_members)
            }
            SvalinGroupId::GlobalGroup => Ok(vec![self.root.spki_hash().clone()]),
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
