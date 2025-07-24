use std::{fmt::Debug, sync::Arc};

use anyhow::Result;

use rcgen::{IsCa, Issuer};
use ring::signature::Ed25519KeyPair;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use tracing::debug;
use zeroize::ZeroizeOnDrop;

use crate::{
    Certificate, CertificateParseError, CertificateRequest, KeyPair,
    encrypt::EncryptedObject,
    keypair::{DecodeKeypairError, ExportedPublicKey, SavedKeypair},
    signed_message::{CanSign, CanVerify},
};

#[derive(Debug)]
struct CredentialData {
    keypair: KeyPair,
    certificate: Certificate,
}

#[derive(Clone)]
pub struct Credential {
    data: Arc<CredentialData>,
}

impl Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermCredentials")
            .field("certificate", &self.data.certificate)
            .finish()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedCredentials {
    encrypted_keypair: EncryptedObject<SavedKeypair>,
    raw_cert: Vec<u8>,
}

impl EncryptedCredentials {
    pub async fn decrypt(self, password: Vec<u8>) -> Result<Credential, DecodeCredentialsError> {
        let certificate = Certificate::from_der(self.raw_cert)?;

        debug!("decrypting credentials with password");

        let decrypted_keypair = KeyPair::decrypt(self.encrypted_keypair, password).await?;

        debug!("credentials decrypted");

        Credential::new(decrypted_keypair, certificate)
            .map_err(DecodeCredentialsError::CreateCredentialsError)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateCredentialsError {
    #[error("error self-signing certificate: {0}")]
    SelfSignError(rcgen::Error),
    #[error("error self-signing certificate: {0}")]
    SignCertificateError(rcgen::Error),
    #[error("key rejected: {0}")]
    KeyRejectError(ring::error::KeyRejected),
    #[error("error parsing certificate: {0}")]
    CertificateParseError(#[from] CertificateParseError),
    #[error("error while creating issuer from credential: {0}")]
    IssuerCreateError(rcgen::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeCredentialsError {
    #[error("error decoding credentials: {0}")]
    DecodeStructError(#[from] postcard::Error),
    #[error("error parsing certificate: {0}")]
    ParseCertificateError(#[from] CertificateParseError),
    #[error("error parsing keypair: {0}")]
    DecodeKeypairError(#[from] DecodeKeypairError),
    #[error("error creating credentials: {0}")]
    CreateCredentialsError(#[from] CreateCredentialsError),
}

#[derive(Debug, thiserror::Error)]
pub enum ApproveRequestError {
    #[error("error parsing keypair: {0}")]
    KeypairParseError(rcgen::Error),
    #[error("error creating ca-certificate params: {0}")]
    CreateCaParamsError(rcgen::Error),
    #[error("error creating ca-certificate: {0}")]
    CreateCaError(rcgen::Error),
    #[error("error signing certificate: {0}")]
    SignCertError(rcgen::Error),
    #[error("error parsing new certificate: {0}")]
    ParseNewCertError(CertificateParseError),
}

impl Credential {
    pub(crate) fn new(
        keypair: KeyPair,
        certificate: Certificate,
    ) -> Result<Self, CreateCredentialsError> {
        // TODO: check if keypair and certificate belong together

        // println!("{:?}", keypair.public_key().as_ref());
        // println!("{:?}", certificate.borrow_public_key());

        // if *keypair.public_key().as_ref() != *certificate.borrow_public_key() {
        //     bail!(crate::Error::KeyMismatch)
        // }

        Ok(Credential {
            data: Arc::new(CredentialData {
                keypair,
                certificate,
            }),
        })
    }

    /// Generates a new root certificate with 10 year lifetime and options tuned for svalin
    pub fn generate_root() -> Result<Self, CreateCredentialsError> {
        let mut root_parameters = rcgen::CertificateParams::default();
        root_parameters.not_before = OffsetDateTime::now_utc();
        root_parameters.not_after =
            OffsetDateTime::now_utc().saturating_add(Duration::days(365 * 10));
        root_parameters.distinguished_name = rcgen::DistinguishedName::new();

        root_parameters.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);

        root_parameters.key_usages = vec![
            rcgen::KeyUsagePurpose::DigitalSignature,
            rcgen::KeyUsagePurpose::KeyCertSign,
            // I'm unsure if I should set this, as certificate revocation does not use one of the official revocation methods
            // rcgen::KeyUsagePurpose::CrlSign,
        ];

        root_parameters.use_authority_key_identifier_extension = true;
        root_parameters.key_identifier_method = rcgen::KeyIdMethod::Sha256;

        let keypair = KeyPair::generate();

        let certificate = root_parameters
            .self_signed(keypair.rcgen())
            .map_err(CreateCredentialsError::SelfSignError)?;

        let certificate = Certificate::from_der(certificate.der().to_vec())?;

        let root = Self::new(keypair, certificate)?;

        Ok(root)
    }

    fn issuer<'a>(&'a self) -> Result<Issuer<'a, rcgen::KeyPair>, rcgen::Error> {
        Issuer::from_ca_cert_der(
            &self.data.certificate.to_der().into(),
            self.data.keypair.rcgen_clone(),
        )
    }

    /// Creates a certificate with the given public key.
    /// Svalin uses these certificates as device credentials.
    pub fn create_leaf_certificate_for_key(
        &self,
        public_key: &ExportedPublicKey,
    ) -> Result<Certificate, CreateCredentialsError> {
        let mut leaf_parameters = rcgen::CertificateParams::default();
        leaf_parameters.not_before = OffsetDateTime::now_utc();
        leaf_parameters.not_after = OffsetDateTime::now_utc().saturating_add(Duration::days(365));
        leaf_parameters.distinguished_name = rcgen::DistinguishedName::new();

        leaf_parameters.is_ca = rcgen::IsCa::NoCa;

        leaf_parameters.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];

        leaf_parameters.use_authority_key_identifier_extension = true;
        leaf_parameters.key_identifier_method = rcgen::KeyIdMethod::Sha256;

        let certificate = leaf_parameters
            .signed_by(
                public_key,
                &self
                    .issuer()
                    .map_err(CreateCredentialsError::IssuerCreateError)?,
            )
            .map_err(CreateCredentialsError::SignCertificateError)?;

        let leaf = Certificate::from_der(certificate.der().to_vec())?;

        Ok(leaf)
    }

    pub async fn export(&self, password: Vec<u8>) -> Result<EncryptedCredentials> {
        let encrypted_keypair = self.data.keypair.encrypt(password).await?;
        let on_disk = EncryptedCredentials {
            encrypted_keypair,
            raw_cert: self.data.certificate.to_der().to_owned(),
        };

        Ok(on_disk)
    }

    pub fn get_certificate(&self) -> &Certificate {
        &self.data.certificate
    }
}

impl CanSign for Credential {
    fn borrow_keypair(&self) -> &Ed25519KeyPair {
        &self.data.keypair.borrow_keypair()
    }
}

impl CanVerify for Credential {
    fn borrow_public_key(&self) -> &[u8] {
        self.data.keypair.borrow_public_key()
    }
}

#[cfg(test)]
mod test {
    use ring::rand::{SecureRandom, SystemRandom};

    use crate::Credential;

    #[tokio::test]
    async fn test_on_disk_storage() {
        let original = Credential::generate_root().unwrap();

        let rand = SystemRandom::new();

        let mut pw_seed = [0u8; 32];
        rand.fill(&mut pw_seed).unwrap();
        let pw = String::from_utf8(
            pw_seed
                .iter()
                .map(|rand_num| (*rand_num & 0b00011111u8) + 58u8)
                .collect(),
        )
        .unwrap();

        let encrypted_credentials = original.export(pw.clone().into()).await.unwrap();

        let copy = encrypted_credentials.decrypt(pw.into()).await.unwrap();

        assert_eq!(
            copy.data.keypair.export_public_key(),
            original.data.keypair.export_public_key()
        );
        assert_eq!(copy.data.certificate, original.data.certificate);
    }
}
