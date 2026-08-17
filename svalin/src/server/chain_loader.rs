use std::{
    fmt::Debug,
    sync::{Arc, RwLock},
};

use svalin_pki::{
    CertificateChainBuilder, SpkiHash, UnverifiedCertificateChain,
    trust_store::{self, TrustStore},
};
use svalin_server_store::{SessionStore, UserStore};

#[derive(Debug, Clone)]
pub struct ChainLoader {
    trust_store: Arc<RwLock<TrustStore>>,
    session_store: Arc<SessionStore>,
}

impl ChainLoader {
    pub fn new(trust_store: Arc<RwLock<TrustStore>>, session_store: Arc<SessionStore>) -> Self {
        Self {
            trust_store,
            session_store,
        }
    }
}

impl ChainLoader {
    pub async fn load_certificate_chain(
        &self,
        request: &SpkiHash,
    ) -> Result<Option<UnverifiedCertificateChain>, anyhow::Error> {
        let certificate = self.session_store.get_session(request).await?;
        let trust_store = self.trust_store.read().unwrap();
        let certificate = match certificate {
            Some(session) => Some(session),
            None => match trust_store.get(request) {
                Some(cert) => Some(cert.clone().to_unverified()),
                None => None,
            },
        };

        let Some(certificate) = certificate else {
            return Ok(None);
        };

        let cert_chain = CertificateChainBuilder::new(certificate);

        let cert_chain = trust_store.complete_certificate_chain(cert_chain)?;

        Ok(Some(cert_chain))
    }
}
