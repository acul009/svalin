use crate::{Certificate, SpkiHash};

use super::{VerificationError, Verifier};

#[derive(Debug)]
pub struct ExactVerififier {
    expected: Certificate,
    expected_spki_hash: SpkiHash,
}

impl ExactVerififier {
    pub fn new(expected: Certificate) -> Self {
        let expected_spki_hash = expected.spki_hash().clone();
        Self {
            expected,
            expected_spki_hash: expected_spki_hash,
        }
    }
}

impl Verifier for ExactVerififier {
    fn verify_fingerprint(
        &self,
        fingerprint: &SpkiHash,
        time: u64,
    ) -> impl std::future::Future<Output = Result<crate::Certificate, VerificationError>> + Send
    {
        async move {
            if fingerprint == &self.expected_spki_hash {
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
