use std::sync::Arc;

use crate::Certificate;

use super::{VerificationError, Verifier};

#[derive(Debug)]
pub struct ExactVerififier {
    expected: Certificate,
}

impl ExactVerififier {
    pub fn new(expected: Certificate) -> Self {
        Self { expected }
    }
}

impl Verifier for ExactVerififier {
    fn verify_fingerprint(
        &self,
        fingerprint: spki::FingerprintBytes,
        time: u64,
    ) -> impl std::future::Future<Output = Result<crate::Certificate, VerificationError>> + Send
    {
        async move {
            if fingerprint == self.expected.get_fingerprint() {
                self.expected
                    .check_validity_at(time)
                    .map_err(|err| VerificationError::TimerangeError(err))?;

                Ok(self.expected.clone())
            } else {
                Err(VerificationError::CertificateMismatch)
            }
        }
    }
}
