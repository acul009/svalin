use std::{error::Error, fmt::Display, future::Future};

use crate::{certificate::CertificateFingerprint, Certificate};

#[derive(Debug)]
enum VerificationError {
    CertificateRevoked,
    CertificateInvalid,
    UnknownCertificate,
    FingerprintCollission {
        fingerprint: CertificateFingerprint,
        given_cert: Certificate,
        loaded_cert: Certificate,
    },
}

impl Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationError::CertificateRevoked => write!(f, "The certificate of the given fingerprint was revoked"),
            VerificationError::CertificateInvalid => write!(f, "The certificate of the given fingerprint is invalid"),
            VerificationError::UnknownCertificate => write!(f, "The certificate corresponding to the given fingerprint is unknown"),
            VerificationError::FingerprintCollission { fingerprint, given_cert, loaded_cert } => write!(f, "The given fingerprint {} is shared between these two certificates: {:?} (given) vs {:?} (loaded)", fingerprint, given_cert, loaded_cert),
        }
    }
}

impl Error for VerificationError {}

pub trait Verifier {
    fn verify_fingerprint(
        &self,
        fingerprint: CertificateFingerprint,
    ) -> impl Future<Output = Result<Certificate, VerificationError>>;
}

impl<T: Verifier> KnownCertificateVerifier for T {}

pub trait KnownCertificateVerifier: Verifier {
    fn verify_known_certificate(
        &self,
        cert: &Certificate,
    ) -> impl Future<Output = Result<(), VerificationError>> {
        async move {
            let fingerprint = cert.get_fingerprint();

            let loaded_cert = self.verify_fingerprint(fingerprint).await?;

            if cert != &loaded_cert {
                Err(VerificationError::FingerprintCollission {
                    fingerprint,
                    loaded_cert,
                    given_cert: cert.clone(),
                })
            } else {
                Ok(())
            }
        }
    }
}
