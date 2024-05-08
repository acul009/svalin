use std::sync::Arc;

use anyhow::{anyhow, Result};
use quinn::rustls::{client::danger::ServerCertVerified, crypto::CryptoProvider};

// Implementation of `ServerCertVerifier` that verifies everything as trustworthy.
#[derive(Debug)]
pub struct SkipServerVerification(Arc<CryptoProvider>);

impl SkipServerVerification {
    pub fn new() -> Arc<Self> {
        let provider = crate::defaults::crypto_provider();
        Arc::new(Self(provider))
    }
}

impl quinn::rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        end_entity: &quinn::rustls::pki_types::CertificateDer<'_>,
        intermediates: &[quinn::rustls::pki_types::CertificateDer<'_>],
        server_name: &quinn::rustls::pki_types::ServerName<'_>,
        ocsp_response: &[u8],
        now: quinn::rustls::pki_types::UnixTime,
    ) -> Result<quinn::rustls::client::danger::ServerCertVerified, quinn::rustls::Error> {
        Ok(ServerCertVerified::assertion())
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
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<quinn::rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[derive(Debug)]
pub struct SkipClientVerification(Arc<CryptoProvider>);

impl SkipClientVerification {
    pub fn new() -> Arc<Self> {
        let provider = crate::defaults::crypto_provider();
        Arc::new(Self(provider))
    }
}

impl quinn::rustls::server::danger::ClientCertVerifier for SkipClientVerification {
    fn root_hint_subjects(&self) -> &[quinn::rustls::DistinguishedName] {
        todo!()
    }

    fn verify_client_cert(
        &self,
        end_entity: &quinn::rustls::pki_types::CertificateDer<'_>,
        intermediates: &[quinn::rustls::pki_types::CertificateDer<'_>],
        now: quinn::rustls::pki_types::UnixTime,
    ) -> Result<quinn::rustls::server::danger::ClientCertVerified, quinn::rustls::Error> {
        todo!()
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &quinn::rustls::pki_types::CertificateDer<'_>,
        dss: &quinn::rustls::DigitallySignedStruct,
    ) -> Result<quinn::rustls::client::danger::HandshakeSignatureValid, quinn::rustls::Error> {
        todo!()
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &quinn::rustls::pki_types::CertificateDer<'_>,
        dss: &quinn::rustls::DigitallySignedStruct,
    ) -> Result<quinn::rustls::client::danger::HandshakeSignatureValid, quinn::rustls::Error> {
        todo!()
    }

    fn supported_verify_schemes(&self) -> Vec<quinn::rustls::SignatureScheme> {
        todo!()
    }
}
