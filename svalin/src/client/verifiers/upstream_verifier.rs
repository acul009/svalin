use std::sync::Arc;

use anyhow::{anyhow, Result};

#[derive(Debug)]
pub struct UpstreamVerifier {
    provider: Arc<quinn::rustls::crypto::CryptoProvider>,
    root_certificate: svalin_pki::Certificate,
    upstream_certificate: svalin_pki::Certificate,
}

impl UpstreamVerifier {
    pub fn new(
        root_certificate: svalin_pki::Certificate,
        upstream_certificate: svalin_pki::Certificate,
    ) -> Arc<Self> {
        let provider = svalin_rpc::defaults::crypto_provider();
        Arc::new(Self {
            root_certificate,
            upstream_certificate,
            provider,
        })
    }
}

impl quinn::rustls::client::danger::ServerCertVerifier for UpstreamVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &quinn::rustls::pki_types::CertificateDer<'_>,
        intermediates: &[quinn::rustls::pki_types::CertificateDer<'_>],
        server_name: &quinn::rustls::pki_types::ServerName<'_>,
        ocsp_response: &[u8],
        now: quinn::rustls::pki_types::UnixTime,
    ) -> Result<quinn::rustls::client::danger::ServerCertVerified, quinn::rustls::Error> {
        if self.upstream_certificate.to_der() != end_entity.as_ref() {
            return Err(quinn::rustls::Error::InvalidCertificate(
                quinn::rustls::CertificateError::ApplicationVerificationFailure,
            ));
        }

        // TODO: check that certificate chain only contains the root and is valid
        Ok(quinn::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &quinn::rustls::pki_types::CertificateDer<'_>,
        dss: &quinn::rustls::DigitallySignedStruct,
    ) -> Result<quinn::rustls::client::danger::HandshakeSignatureValid, quinn::rustls::Error> {
        Err(quinn::rustls::Error::PeerIncompatible(
            quinn::rustls::PeerIncompatible::ServerTlsVersionIsDisabledByOurConfig,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &quinn::rustls::pki_types::CertificateDer<'_>,
        dss: &quinn::rustls::DigitallySignedStruct,
    ) -> Result<quinn::rustls::client::danger::HandshakeSignatureValid, quinn::rustls::Error> {
        quinn::rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<quinn::rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
