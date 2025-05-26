use std::{fmt::Debug, sync::Arc};

use anyhow::Result;

use ring::signature::Ed25519KeyPair;
use serde::{Deserialize, Serialize};
use tracing::debug;
use zeroize::ZeroizeOnDrop;

use crate::{
    Certificate, CertificateParseError, CertificateRequest, Keypair,
    encrypt::EncryptedObject,
    keypair::{DecodeKeypairError, EncryptedKeypair},
    signed_message::{CanSign, CanVerify},
};

#[derive(Debug)]
struct PermCredentialData {
    keypair: Keypair,
    certificate: Certificate,
}

impl ZeroizeOnDrop for PermCredentialData {}

#[derive(Clone)]
pub struct PermCredentials {
    data: Arc<PermCredentialData>,
}

impl Debug for PermCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermCredentials")
            .field("certificate", &self.data.certificate)
            .finish()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedCredentials {
    encrypted_keypair: EncryptedObject<EncryptedKeypair>,
    raw_cert: Vec<u8>,
}

impl EncryptedCredentials {
    pub async fn decrypt(
        self,
        password: Vec<u8>,
    ) -> Result<PermCredentials, DecodeCredentialsError> {
        let certificate = Certificate::from_der(self.raw_cert)?;

        debug!("decrypting credentials with password");

        let decrypted_keypair = Keypair::decrypt(self.encrypted_keypair, password).await?;

        debug!("credentials decrypted");

        PermCredentials::new(decrypted_keypair, certificate)
            .map_err(DecodeCredentialsError::CreateCredentialsError)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateCredentialsError {
    #[error("key rejected: {0}")]
    KeyRejectError(ring::error::KeyRejected),
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

impl PermCredentials {
    pub(crate) fn new(
        keypair: Keypair,
        certificate: Certificate,
    ) -> Result<Self, CreateCredentialsError> {
        // TODO: check if keypair and certificate belong together

        // println!("{:?}", keypair.public_key().as_ref());
        // println!("{:?}", certificate.borrow_public_key());

        // if *keypair.public_key().as_ref() != *certificate.borrow_public_key() {
        //     bail!(crate::Error::KeyMismatch)
        // }

        Ok(PermCredentials {
            data: Arc::new(PermCredentialData {
                keypair,
                certificate,
            }),
        })
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

    pub fn get_der_key_bytes(&self) -> &[u8] {
        &self.data.keypair.get_der_key_bytes()
    }

    pub fn approve_request(
        &self,
        request: CertificateRequest,
    ) -> Result<Certificate, ApproveRequestError> {
        let ca_params =
            rcgen::CertificateParams::from_ca_cert_der(&self.data.certificate.to_der().into())
                .map_err(ApproveRequestError::CreateCaParamsError)?;

        let ca = ca_params
            .self_signed(self.data.keypair.rcgen())
            .map_err(ApproveRequestError::CreateCaError)?;

        let rcgen_cert = request
            .csr
            .signed_by(&ca, self.data.keypair.rcgen())
            .map_err(ApproveRequestError::SignCertError)?;

        let new_cert = Certificate::from_der(rcgen_cert.der().to_vec())
            .map_err(ApproveRequestError::ParseNewCertError)?;

        Ok(new_cert)
    }
}

impl CanSign for PermCredentials {
    fn borrow_keypair(&self) -> &Ed25519KeyPair {
        &self.data.keypair.borrow_keypair()
    }
}

impl CanVerify for PermCredentials {
    fn borrow_public_key(&self) -> &[u8] {
        self.data.keypair.borrow_public_key()
    }
}

#[cfg(test)]
mod test {
    use ring::rand::{SecureRandom, SystemRandom};

    use crate::Keypair;

    #[tokio::test]
    async fn test_on_disk_storage() {
        let original = Keypair::generate().to_self_signed_cert().unwrap();

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
            copy.data.keypair.spki_hash(),
            original.data.keypair.spki_hash()
        );
        assert_eq!(copy.data.certificate, original.data.certificate);
    }
}
