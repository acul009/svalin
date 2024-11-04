use svalin_pki::{verifier::VerificationError, Certificate};

#[derive(Debug)]
pub struct VerificationHelper {
    root: Certificate,
}

impl VerificationHelper {
    pub fn new(root: Certificate) -> Self {
        Self { root }
    }

    pub fn try_root(&self, fingerprint: [u8; 32]) -> Option<Certificate> {
        if fingerprint == self.root.get_fingerprint() {
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

        // TODO: check revocation

        // TODO: check certificate chain

        todo!()
    }
}
