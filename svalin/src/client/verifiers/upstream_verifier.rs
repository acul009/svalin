use std::sync::Arc;

use svalin_rpc::rustls;

use anyhow::Result;

#[derive(Debug)]
pub struct UpstreamVerifier {
    provider: Arc<rustls::crypto::CryptoProvider>,
    _root_certificate: svalin_pki::Certificate,
    upstream_certificate: svalin_pki::Certificate,
}

impl UpstreamVerifier {
    pub fn new(
        root_certificate: svalin_pki::Certificate,
        upstream_certificate: svalin_pki::Certificate,
    ) -> Arc<Self> {
        let provider = svalin_rpc::defaults::crypto_provider();
        Arc::new(Self {
            _root_certificate: root_certificate,
            upstream_certificate,
            provider,
        })
    }
}

impl rustls::client::danger::ServerCertVerifier for UpstreamVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        if self.upstream_certificate.to_der() != end_entity.as_ref() {
            return Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ));
        }

        // TODO: check that certificate chain only contains the root and is valid
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::PeerIncompatible(
            rustls::PeerIncompatible::ServerTlsVersionIsDisabledByOurConfig,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
