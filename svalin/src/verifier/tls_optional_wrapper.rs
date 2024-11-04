use std::sync::Arc;

use svalin_rpc::rustls::server::danger::ClientCertVerifier;

#[derive(Clone, Debug)]
pub struct TlsOptionalWrapper<T> {
    inner: Arc<T>,
}

impl<T> TlsOptionalWrapper<T> {
    pub fn new(inner: Arc<T>) -> Arc<Self> {
        Arc::new(Self { inner })
    }
}

impl<T> ClientCertVerifier for TlsOptionalWrapper<T>
where
    T: ClientCertVerifier,
{
    fn client_auth_mandatory(&self) -> bool {
        false
    }

    fn root_hint_subjects(&self) -> &[svalin_rpc::rustls::DistinguishedName] {
        self.inner.root_hint_subjects()
    }

    fn verify_client_cert(
        &self,
        end_entity: &svalin_rpc::rustls::pki_types::CertificateDer<'_>,
        intermediates: &[svalin_rpc::rustls::pki_types::CertificateDer<'_>],
        now: svalin_rpc::rustls::pki_types::UnixTime,
    ) -> Result<svalin_rpc::rustls::server::danger::ClientCertVerified, svalin_rpc::rustls::Error>
    {
        self.inner
            .verify_client_cert(end_entity, intermediates, now)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &svalin_rpc::rustls::pki_types::CertificateDer<'_>,
        dss: &svalin_rpc::rustls::DigitallySignedStruct,
    ) -> Result<
        svalin_rpc::rustls::client::danger::HandshakeSignatureValid,
        svalin_rpc::rustls::Error,
    > {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &svalin_rpc::rustls::pki_types::CertificateDer<'_>,
        dss: &svalin_rpc::rustls::DigitallySignedStruct,
    ) -> Result<
        svalin_rpc::rustls::client::danger::HandshakeSignatureValid,
        svalin_rpc::rustls::Error,
    > {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<svalin_rpc::rustls::SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}
