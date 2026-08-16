use crate::{CertificateType, Verifier};

#[derive(Debug)]
pub struct TypeLimitedVerifier<V: Verifier> {
    inner: V,
    check: fn(CertificateType) -> bool,
}

pub trait Limit: Verifier + Sized {
    fn server_only(self) -> TypeLimitedVerifier<Self>;
}

impl<V: Verifier> Limit for V {
    fn server_only(self) -> TypeLimitedVerifier<Self> {
        TypeLimitedVerifier {
            inner: self,
            check: |cert_type| match cert_type {
                CertificateType::Server => true,
                _ => false,
            },
        }
    }
}

impl<V: Verifier> Verifier for TypeLimitedVerifier<V> {
    async fn verify_spki_hash(
        &self,
        spki_hash: &crate::SpkiHash,
        time: u64,
    ) -> Result<crate::Certificate, super::VerifyError> {
        let cert = self.inner.verify_spki_hash(spki_hash, time).await?;
        if (self.check)(cert.certificate_type()) {
            Ok(cert)
        } else {
            Err(super::VerifyError::IncorrectCertificateType)
        }
    }
}
