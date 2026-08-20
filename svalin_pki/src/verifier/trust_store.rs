use std::sync::{Arc, RwLock};

use crate::{CertificateType, trust_store::TrustStore};

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

    async fn verify_known_certificate(
        &self,
        cert: &crate::UnverifiedCertificate,
        time: u64,
    ) -> Result<crate::Certificate, super::VerifyError> {
        match cert.certificate_type() {
            CertificateType::Temporary => Err(super::VerifyError::IncorrectCertificateType),
            CertificateType::Root
            | CertificateType::Agent
            | CertificateType::Server
            | CertificateType::User => {
                let correct_cert = self.verify_spki_hash(cert.spki_hash(), time).await?;
                if correct_cert == *cert {
                    Ok(correct_cert)
                } else {
                    Err(super::VerifyError::IncorrectCertificateType)
                }
            }
            CertificateType::UserSession => {
                let issuer = self.verify_spki_hash(cert.issuer(), time).await?;
                let cert = cert.clone().verify_signature(&issuer, time)?;
                // TODO: Session revocation
                Ok(cert)
            }
        }
    }
}
