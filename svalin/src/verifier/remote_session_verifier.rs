use std::fmt::Debug;

use svalin_pki::{CertificateType, SpkiHash, Verifier, VerifyError};

use crate::verifier::remote_verifier::RemoteVerifier;

pub struct RemoteSessionVerifier {
    inner: RemoteVerifier,
}

impl Debug for RemoteSessionVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSessionVerifier").finish()
    }
}

impl RemoteSessionVerifier {
    pub fn new(inner: RemoteVerifier) -> Self {
        Self { inner }
    }
}

impl Verifier for RemoteSessionVerifier {
    async fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> Result<svalin_pki::Certificate, svalin_pki::VerifyError> {
        let certificate = self.inner.verify_spki_hash(spki_hash, time).await?;

        if certificate.certificate_type() == CertificateType::UserSession {
            Ok(certificate)
        } else {
            Err(VerifyError::IncorrectCertificateType)
        }
    }
}
