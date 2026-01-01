use openmls::prelude::{
    KeyPackage, KeyPackageIn, KeyPackageVerifyError, OpenMlsCrypto, ProtocolVersion,
};
use serde::{Deserialize, Serialize};

use crate::{
    Certificate, KnownCertificateVerifier, SpkiHash, Verifier, VerifyError,
    certificate::UnverifiedCertificate, get_current_timestamp,
};

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
    CertificateVerificationError(#[from] VerifyError),
}

impl UnverifiedNewMember {
    pub async fn verify(
        self,
        crypto: &impl OpenMlsCrypto,
        protocol_version: ProtocolVersion,
        verifier: &impl Verifier,
    ) -> Result<NewMember, VerifyNewMemberError> {
        let key_package = self.key_package.validate(crypto, protocol_version)?;

        let unverified_cert: UnverifiedCertificate =
            key_package.leaf_node().credential().deserialized()?;

        let signature_key = key_package.leaf_node().signature_key();
        if signature_key.as_slice() != unverified_cert.public_key() {
            return Err(VerifyNewMemberError::SignatureKeyMismatch);
        }

        let verified_cert = verifier
            .verify_known_certificate(&unverified_cert, get_current_timestamp())
            .await?;

        Ok(NewMember {
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
