use crate::{Certificate, SpkiHash};

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
    fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> impl std::future::Future<Output = Result<crate::Certificate, VerificationError>> + Send
    {
        async move {
            if spki_hash == self.expected.spki_hash() {
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
