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

    pub async fn help_verify(
        &self,
        time: u64,
        cert: Certificate,
    ) -> Result<Certificate, VerificationError> {
        cert.check_validity_at(time)?;

        if cert == self.root {
            return Ok(cert);
        }

        let Some(chain) = self
            .user_store
            .get_verified_user_chain_by_spki_hash(cert.issuer())
            .await?
        else {
            return Err(VerificationError::UnknownIssuer);
        };

        for certificate in &chain {
            certificate.check_validity_at(time).map_err(|err| {
                VerificationError::ChainTimerangeError(err, certificate.fingerprint().clone())
            })?;
        }

        cert.verify_signature(
            chain
                .first()
                .expect("A chain must have at least one certificate"),
        )?;

        // TODO: check revocation

        Ok(cert)
    }
}
