use std::{error::Error, fmt::Display, future::Future};

use spki::FingerprintBytes;
use time::Time;

use crate::Certificate;

#[derive(Debug)]
pub enum VerificationError {
    CertificateRevoked,
    CertificateInvalid,
    UnknownCertificate,
    FingerprintCollission {
        fingerprint: FingerprintBytes,
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
            VerificationError::FingerprintCollission { fingerprint, given_cert, loaded_cert } => write!(f, "The given fingerprint {:x?} is shared between these two certificates: {:?} (given) vs {:?} (loaded)", fingerprint, given_cert, loaded_cert),
        }
    }
}

impl Error for VerificationError {}

pub trait Verifier: Send + Sync {
    /// TODO: include time for revocation/expiration checking
    fn verify_fingerprint(
        &self,
        fingerprint: FingerprintBytes,
        time: u64,
    ) -> impl Future<Output = Result<Certificate, VerificationError>> + Send;
}

impl<T: Verifier> KnownCertificateVerifier for T {}

pub trait KnownCertificateVerifier: Verifier {
    fn verify_known_certificate(
        &self,
        cert: &Certificate,
        time: u64,
    ) -> impl Future<Output = Result<(), VerificationError>> + Send {
        async move {
            let fingerprint = cert.get_fingerprint();

            let loaded_cert = self.verify_fingerprint(fingerprint, time).await?;

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

#[cfg(feature = "rustls")]
pub use rustls::*;

#[cfg(feature = "rustls")]
mod rustls {

    use std::{
        fmt::Debug,
        sync::{Arc, LazyLock},
    };

    use rustls::{
        client::danger::ServerCertVerified, crypto::CryptoProvider, CertificateError, OtherError,
    };
    use tokio::task::block_in_place;

    use crate::{
        verifier::{self, VerificationError},
        Certificate,
    };

    use super::KnownCertificateVerifier;

    static CRYPTO_PROVIDER: LazyLock<Arc<rustls::crypto::CryptoProvider>> =
        LazyLock::new(|| Arc::new(rustls::crypto::ring::default_provider()));

    pub fn crypto_provider() -> Arc<rustls::crypto::CryptoProvider> {
        CRYPTO_PROVIDER.clone()
    }

    #[derive(Debug)]
    pub struct RustlsVerifier<T> {
        verifier: Arc<T>,
        cryptoprovider: Arc<CryptoProvider>,
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
                .map_err(|err| rustls::Error::InvalidCertificate(CertificateError::BadEncoding))?;

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
    }

    impl<T> rustls::client::danger::ServerCertVerifier for RustlsVerifier<T>
    where
        T: Debug + KnownCertificateVerifier + 'static,
    {
        fn verify_server_cert(
            &self,
            end_entity: &rustls::pki_types::CertificateDer<'_>,
            intermediates: &[rustls::pki_types::CertificateDer<'_>],
            server_name: &rustls::pki_types::ServerName<'_>,
            ocsp_response: &[u8],
            now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            self.verify_cert(end_entity, now)
                .map(|_| ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &rustls::pki_types::CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
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
                &self.cryptoprovider.signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            self.cryptoprovider
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
            intermediates: &[rustls::pki_types::CertificateDer<'_>],
            now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
            self.verify_cert(end_entity, now)
                .map(|_| rustls::server::danger::ClientCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &rustls::pki_types::CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
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
                &self.cryptoprovider.signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            self.cryptoprovider
                .signature_verification_algorithms
                .supported_schemes()
        }
    }
}
