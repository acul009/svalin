use crate::{Certificate, certificate::Fingerprint};

use super::{VerificationError, Verifier};

#[derive(Debug)]
pub struct ExactVerififier {
    expected: Certificate,
    expected_fingerprint: Fingerprint,
}

impl ExactVerififier {
    pub fn new(expected: Certificate) -> Self {
        let expected_fingerprint = expected.fingerprint().clone();
        Self {
            expected,
            expected_fingerprint,
        }
    }
}

impl Verifier for ExactVerififier {
    fn verify_fingerprint(
        &self,
        fingerprint: &Fingerprint,
        time: u64,
    ) -> impl std::future::Future<Output = Result<crate::Certificate, VerificationError>> + Send
    {
        async move {
            if fingerprint == &self.expected_fingerprint {
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
