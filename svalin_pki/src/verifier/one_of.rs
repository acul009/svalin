use crate::{Certificate, SpkiHash};
use std::collections::HashMap;

use super::{Verifier, VerifyError};

#[derive(Debug)]
pub struct OneOfVerififier {
    expected: HashMap<SpkiHash, Certificate>,
}

impl OneOfVerififier {
    pub fn new(expected: impl Iterator<Item = Certificate>) -> Self {
        Self {
            expected: expected
                .map(|cert| (cert.spki_hash().clone(), cert))
                .collect(),
        }
    }
}

impl Verifier for OneOfVerififier {
    fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> impl std::future::Future<Output = Result<crate::Certificate, VerifyError>> + Send {
        async move {
            if let Some(cert) = self.expected.get(spki_hash) {
                cert.check_validity_at(time)
                    .map_err(|err| VerifyError::TimerangeError(err))?;

                Ok(cert.clone())
            } else {
                Err(VerifyError::CertificateMismatch)
            }
        }
    }
}
