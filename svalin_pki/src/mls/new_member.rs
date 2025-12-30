use std::time::{SystemTime, UNIX_EPOCH};

use openmls::prelude::{
    KeyPackage, KeyPackageIn, KeyPackageVerifyError, OpenMlsCrypto, ProtocolVersion,
};
use serde::{Deserialize, Serialize};

use crate::{Certificate, KnownCertificateVerifier, SpkiHash, VerificationError, Verifier};

#[derive(Serialize, Deserialize)]
pub struct UnverifiedNewMember {
    key_package: KeyPackageIn,
}

pub struct NewMember {
    key_package: KeyPackage,
    certificate: Certificate,
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyNewMemberError {
    #[error("KeyPackage verification error: {0}")]
    VerifyError(#[from] KeyPackageVerifyError),
    #[error("Certificate deserialization error: {0}")]
    DeserializeCertError(#[from] tls_codec::Error),
    #[error("Signature key mismatch")]
    SignatureKeyMismatch,
    #[error("Certificate verification error: {0}")]
    CertificateVerificationError(#[from] VerificationError),
}

impl UnverifiedNewMember {
    pub async fn verify(
        self,
        crypto: &impl OpenMlsCrypto,
        protocol_version: ProtocolVersion,
        verifier: &impl Verifier,
    ) -> Result<NewMember, VerifyNewMemberError> {
        let key_package = self.key_package.validate(crypto, protocol_version)?;

        let cert: Certificate = key_package.leaf_node().credential().deserialized()?;

        let signature_key = key_package.leaf_node().signature_key();
        if signature_key.as_slice() != cert.public_key() {
            return Err(VerifyNewMemberError::SignatureKeyMismatch);
        }

        verifier
            .verify_known_certificate(
                &cert,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("We should never be before the unix epoch")
                    .as_secs(),
            )
            .await?;

        Ok(NewMember {
            key_package,
            certificate: cert,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeserializeNewMemberError {
    #[error("TLS codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
}

impl NewMember {
    pub fn to_unverified(self) -> UnverifiedNewMember {
        UnverifiedNewMember {
            key_package: self.key_package.into(),
        }
    }

    pub fn spki_hash(&self) -> &SpkiHash {
        self.certificate.spki_hash()
    }

    pub(crate) fn to_key_package(self) -> KeyPackage {
        self.key_package
    }
}
