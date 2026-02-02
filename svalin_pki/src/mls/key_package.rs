use openmls::prelude::{KeyPackageIn, KeyPackageVerifyError, OpenMlsCrypto, ProtocolVersion};
use serde::{Deserialize, Serialize};

use crate::{
    Certificate, CertificateType, KnownCertificateVerifier, SpkiHash, Verifier, VerifyError,
    certificate::UnverifiedCertificate, get_current_timestamp,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct UnverifiedKeyPackage {
    key_package: KeyPackageIn,
}

pub struct KeyPackage {
    key_package: openmls::prelude::KeyPackage,
    certificate: Certificate,
}

#[derive(Debug, thiserror::Error)]
pub enum KeyPackageError {
    #[error("KeyPackage verification error: {0}")]
    VerifyError(#[from] KeyPackageVerifyError),
    #[error("Certificate deserialization error: {0}")]
    DeserializeCertError(#[from] tls_codec::Error),
    #[error("Signature key mismatch")]
    SignatureKeyMismatch,
    #[error("Certificate verification error: {0}")]
    CertificateVerificationError(#[from] VerifyError),
    #[error("Certificate mismatch")]
    CertificateMismatch,
}

impl UnverifiedKeyPackage {
    pub async fn verify(
        self,
        crypto: &impl OpenMlsCrypto,
        protocol_version: ProtocolVersion,
        verifier: &impl Verifier,
    ) -> Result<KeyPackage, KeyPackageError> {
        let key_package = self.key_package.validate(crypto, protocol_version)?;

        let unverified_cert: UnverifiedCertificate =
            key_package.leaf_node().credential().deserialized()?;

        let signature_key = key_package.leaf_node().signature_key();
        if signature_key.as_slice() != unverified_cert.public_key() {
            return Err(KeyPackageError::SignatureKeyMismatch);
        }

        let verified_cert = verifier
            .verify_known_certificate(&unverified_cert, get_current_timestamp())
            .await?;

        Ok(KeyPackage {
            key_package,
            certificate: verified_cert,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeserializeNewMemberError {
    #[error("TLS codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
}

impl KeyPackage {
    pub(crate) fn new(
        certificate: Certificate,
        key_package: openmls::prelude::KeyPackage,
    ) -> Result<Self, KeyPackageError> {
        let unverified_cert: UnverifiedCertificate =
            key_package.leaf_node().credential().deserialized()?;

        if unverified_cert != certificate {
            return Err(KeyPackageError::CertificateMismatch);
        }

        Ok(Self {
            key_package,
            certificate,
        })
    }

    pub fn to_unverified(self) -> UnverifiedKeyPackage {
        UnverifiedKeyPackage {
            key_package: self.key_package.into(),
        }
    }

    pub fn spki_hash(&self) -> &SpkiHash {
        self.certificate.spki_hash()
    }

    pub fn user_spki_hash(&self) -> &SpkiHash {
        if self.certificate.certificate_type() == CertificateType::UserDevice {
            self.certificate.issuer()
        } else {
            self.certificate.spki_hash()
        }
    }

    pub fn certificate(&self) -> &Certificate {
        &self.certificate
    }

    pub(crate) fn unpack(self) -> openmls::prelude::KeyPackage {
        self.key_package
    }
}
