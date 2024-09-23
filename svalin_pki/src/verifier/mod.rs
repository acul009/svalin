use std::future::Future;

use crate::{certificate::CertificateHash, Certificate};

enum VerificationError {
    CertificateRevoked,
    CertificateInvalid,
    UnexpectedCertificate,
    UnknownCertificate,
}

// pub trait Verifier {
//     fn verify(
//         &self,
//         cert_hash: CertificateHash,
//     ) -> impl Future<Output = Result<Certificate, VerificationError>>;
// }

// pub trait Verifier {
//     fn verify(&self, cert: &Certificate) -> impl Future<Output = Result<(),
// VerificationError>>; }
