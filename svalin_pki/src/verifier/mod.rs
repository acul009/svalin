use std::{fmt::Debug, future::Future};
use thiserror::Error;

use crate::{
    Certificate,
    certificate::{Fingerprint, SignatureVerificationError, ValidityError},
};

pub mod exact;

#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("The required logic to verify this certificate has not been written yet")]
    NotYetImplemented,
    #[error("The certificate of the given fingerprint was revoked")]
    CertificateRevoked,
    #[error("The certificate of the given fingerprint is invalid")]
    CertificateInvalid,
    #[error("The certificate corresponding to the given fingerprint is unknown")]
    UnknownCertificate,
    #[error("The issuer of the certificate could not be found")]
    UnknownIssuer,
    #[error("The fingerprint did not match the expected certificate")]
    CertificateMismatch,
    #[error("The certificate is not valid: {0}")]
    TimerangeError(#[from] ValidityError),
    #[error("The certificate in the chain with fingerprint {1} is not valid: {0}")]
    ChainTimerangeError(ValidityError, Fingerprint),
    #[error("The signature could not be verified: {0}")]
    SignatureError(#[from] SignatureVerificationError),
    #[error(
        "The given fingerprint {fingerprint:x?} is shared between these two certificates: {given_cert:?} (given) vs {loaded_cert:?} (loaded)"
    )]
    FingerprintCollission {
        fingerprint: Fingerprint,
        given_cert: Certificate,
        loaded_cert: Certificate,
    },
    #[error("Internal Error: {0}")]
    InternalError(#[from] anyhow::Error),
}

pub trait Verifier: Send + Sync + Debug + 'static {
    /// TODO: include time for revocation/expiration checking
    fn verify_fingerprint(
        &self,
        fingerprint: &Fingerprint,
        time: u64,
    ) -> impl Future<Output = Result<Certificate, VerificationError>> + Send;
}

impl<T: Verifier> KnownCertificateVerifier for T {}

pub trait KnownCertificateVerifier: Verifier + Sized + 'static {
    fn verify_known_certificate(
        &self,
        cert: &Certificate,
        time: u64,
    ) -> impl Future<Output = Result<(), VerificationError>> + Send {
        async move {
            let fingerprint = cert.fingerprint();

            let loaded_cert = self.verify_fingerprint(&fingerprint, time).await?;

            if cert != &loaded_cert {
                Err(VerificationError::FingerprintCollission {
                    fingerprint: fingerprint.clone(),
                    loaded_cert,
                    given_cert: cert.clone(),
                })
            } else {
                Ok(())
            }
        }
    }

    fn to_tls_verifier(self) -> std::sync::Arc<rustls_feat::RustlsVerifier<Self>> {
        use std::sync::Arc;

        Arc::new(rustls_feat::RustlsVerifier::new(self))
    }
}

// #[cfg(feature = "rustls")]
// pub use rustls_feat::*;

pub mod rustls_feat {

    use std::{fmt::Debug, sync::Arc};

    use rustls::{
        CertificateError, OtherError, client::danger::ServerCertVerified, crypto::CryptoProvider,
    };
    use tokio::task::block_in_place;

    use crate::{Certificate, verifier::VerificationError};

    use super::KnownCertificateVerifier;

    #[derive(Debug)]
    pub struct RustlsVerifier<T> {
        verifier: Arc<T>,
    }

    impl<T> RustlsVerifier<T> {
        pub fn new(verifier: T) -> Self {
            Self {
                verifier: Arc::new(verifier),
            }
        }
    }

    impl<T> RustlsVerifier<T>
    where
        T: Debug + KnownCertificateVerifier + 'static,
    {
        fn verify_cert(
            &self,
            end_entity: &rustls::pki_types::CertificateDer<'_>,
            now: rustls::pki_types::UnixTime,
        ) -> Result<(), rustls::Error> {
            // TODO: better error handling
            let cert = Certificate::from_der(end_entity.as_ref().to_vec())
                // TODO: wrap error
                .map_err(|_err| rustls::Error::InvalidCertificate(CertificateError::BadEncoding))?;

            let verifier = self.verifier.clone();

            let (send, recv) = std::sync::mpsc::sync_channel(1);

            tokio::spawn(async move {
                send.send(
                    verifier
                        .verify_known_certificate(&cert, now.as_secs())
                        .await,
                )
            });

            let result = block_in_place(move || recv.recv());

            match result {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(VerificationError::CertificateInvalid)) => {
                    Err(CertificateError::BadEncoding)
                }
                Ok(Err(VerificationError::CertificateRevoked)) => Err(CertificateError::Revoked),
                Ok(Err(err)) => Err(CertificateError::Other(OtherError(Arc::new(err)))),
                Err(err) => Err(CertificateError::Other(OtherError(Arc::new(err)))),
            }
            .map_err(|err| rustls::Error::InvalidCertificate(err))
        }

        fn cryptoprovider() -> &'static Arc<CryptoProvider> {
            CryptoProvider::get_default().expect("no CryptoProvider for Rustls installed yet. Please install a default crypto provider: https://docs.rs/rustls/latest/rustls/crypto/struct.CryptoProvider.html")
        }
    }

    impl<T> rustls::client::danger::ServerCertVerifier for RustlsVerifier<T>
    where
        T: Debug + KnownCertificateVerifier + 'static,
    {
        fn verify_server_cert(
            &self,
            end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            self.verify_cert(end_entity, now)
                .map(|_| ServerCertVerified::assertion())
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
                &Self::cryptoprovider().signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            Self::cryptoprovider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    impl<T> rustls::server::danger::ClientCertVerifier for RustlsVerifier<T>
    where
        T: Debug + KnownCertificateVerifier + 'static,
    {
        fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
            &[]
        }

        fn verify_client_cert(
            &self,
            end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
            self.verify_cert(end_entity, now)
                .map(|_| rustls::server::danger::ClientCertVerified::assertion())
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
                &Self::cryptoprovider().signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            Self::cryptoprovider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }
}
