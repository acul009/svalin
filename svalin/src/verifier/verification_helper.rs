use std::sync::Arc;

use svalin_pki::{Certificate, Fingerprint, VerificationError};

use crate::server::user_store::UserStore;

#[derive(Debug)]
pub struct VerificationHelper {
    root: Certificate,
    user_store: Arc<UserStore>,
}

impl VerificationHelper {
    pub fn new(root: Certificate, user_store: Arc<UserStore>) -> Self {
        Self { root, user_store }
    }

    pub fn try_root(&self, fingerprint: &Fingerprint) -> Option<Certificate> {
        if fingerprint == self.root.fingerprint() {
            return Some(self.root.clone());
        }

        None
    }

    pub fn help_verify(
        &self,
        time: u64,
        cert: Certificate,
    ) -> Result<Certificate, VerificationError> {
        cert.check_validity_at(time)?;

        if cert == self.root {
            return Ok(cert);
        }

        if cert.issuer() == self.root.spki_hash() {
            cert.verify_signature(&self.root)?;

            return Ok(cert);
        }

        // TODO: check revocation

        // TODO: check certificate chain

        Err(VerificationError::NotYetImplemented)
    }
}
