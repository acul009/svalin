use std::future::Future;

use crate::{certificate::CertificateHash, Certificate};

enum VerificationError {
    CertificateRevoked,
    CertificateInvalid,
    UnexpectedCertificate,
    UnknownCertificate,
}

pub trait Verifier {
    fn verify_hash(
        &self,
        cert_hash: CertificateHash,
    ) -> impl Future<Output = Result<Certificate, VerificationError>>;

    fn verify_cert(
        &self,
        cert: &Certificate,
    ) -> impl Future<Output = Result<(), VerificationError>>;
}
