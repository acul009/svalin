use std::sync::{Arc, RwLock};

use crate::store::server_store::KeyPackageStore;
use svalin_pki::{
    CertificateChainBuilder, RootCertificate, VerifyChainError, get_current_timestamp,
    mls::{SvalinGroupId, key_retriever::KeyRetriever},
    trust_store::{CompleteCertChainError, TrustStore},
};

#[derive(Debug, thiserror::Error)]
pub enum LocalKeyRetrieverError {
    #[error("agent not found")]
    AgentNotFound,
    #[error("failed to complete certificate chain: {0}")]
    CompleteCertificateChain(#[from] CompleteCertChainError),
    #[error("failed to verify certificate chain: {0}")]
    VerifyCertificateChain(#[from] VerifyChainError),
    #[error("failed to retrieve key packages: {0:#}")]
    GetKeyPackages(anyhow::Error),
}

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
    type Error = LocalKeyRetrieverError;

    async fn get_required_group_members(
        &self,
        id: &SvalinGroupId,
    ) -> Result<Vec<svalin_pki::SpkiHash>, Self::Error> {
        match id {
            SvalinGroupId::DeviceGroup(spki_hash) => {
                let trust_store = self.trust_store.read().unwrap();
                let agent = trust_store
                    .get(&spki_hash)
                    .ok_or(LocalKeyRetrieverError::AgentNotFound)?
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
                    .ok_or(LocalKeyRetrieverError::AgentNotFound)?
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
        }
    }

    async fn get_key_packages(
        &self,
        entities: &[svalin_pki::SpkiHash],
    ) -> Result<Vec<svalin_pki::mls::key_package::UnverifiedKeyPackage>, Self::Error> {
        let key_packages = self
            .key_package_store
            .get_key_packages(entities.iter())
            .await
            .map_err(LocalKeyRetrieverError::GetKeyPackages)?;

        Ok(key_packages)
    }
}
