use std::sync::{Arc, RwLock};

use crate::trust_store::TrustStore;

#[derive(Debug, Clone)]
pub struct TrustStoreVerifier {
    trust_store: Arc<RwLock<TrustStore>>,
}

impl TrustStoreVerifier {
    pub fn new(trust_store: Arc<RwLock<TrustStore>>) -> Self {
        Self { trust_store }
    }
}

impl super::Verifier for TrustStoreVerifier {
    async fn verify_spki_hash(
        &self,
        spki_hash: &crate::SpkiHash,
        time: u64,
    ) -> Result<crate::Certificate, super::VerifyError> {
        let guard = self.trust_store.read().unwrap();
        let Some(cert) = guard.get(spki_hash) else {
            return Err(super::VerifyError::UnknownCertificate);
        };
        cert.check_validity_at(time)?;

        Ok(cert.clone())
    }
}
