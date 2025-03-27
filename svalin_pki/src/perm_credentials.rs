use std::{fmt::Debug, sync::Arc};

use anyhow::{Context, Result};

use ring::signature::{Ed25519KeyPair, KeyPair};
use serde::{Deserialize, Serialize};
use tracing::debug;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{
    Certificate, CertificateParseError, CertificateRequest,
    encrypt::EncryptedData,
    signed_message::{CanSign, CanVerify},
};

#[derive(Debug)]
struct PermCredentialData {
    keypair: Ed25519KeyPair, // Actualy is used below in a trait, compiler is just stupid
    raw_keypair: Vec<u8>,
    certificate: Certificate,
}

impl ZeroizeOnDrop for PermCredentialData {}

impl Drop for PermCredentialData {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl Zeroize for PermCredentialData {
    fn zeroize(&mut self) {
        self.raw_keypair.zeroize();

        // cannot zeroize ring keypair :(
        // self.keypair.zeroize();
    }
}

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

#[derive(Serialize, Deserialize)]
struct CredentialOnDisk {
    encrypted_keypair: Vec<u8>,
    raw_cert: Vec<u8>,
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
    #[error("error decoding keypair: {0}")]
    ParseCertificateError(#[from] CertificateParseError),
    #[error("error decrypting credentials: {0}")]
    DecryptError(#[from] crate::encrypt::DecryptError),
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
        raw_keypair: Vec<u8>,
        certificate: Certificate,
    ) -> Result<Self, CreateCredentialsError> {
        // TODO: check if keypair and certificate belong together

        // println!("{:?}", keypair.public_key().as_ref());
        // println!("{:?}", certificate.borrow_public_key());

        // if *keypair.public_key().as_ref() != *certificate.borrow_public_key() {
        //     bail!(crate::Error::KeyMismatch)
        // }

        let keypair = Ed25519KeyPair::from_pkcs8(&raw_keypair)
            .map_err(CreateCredentialsError::KeyRejectError)?;

        Ok(PermCredentials {
            data: Arc::new(PermCredentialData {
                keypair,
                raw_keypair,
                certificate,
            }),
        })
    }

    pub async fn to_bytes(&self, password: Vec<u8>) -> Result<Vec<u8>> {
        let encrypted_keypair =
            EncryptedData::encrypt_with_password(&self.data.raw_keypair, password)
                .await
                .context("Failed to encrypt keypair")?;
        let on_disk = CredentialOnDisk {
            encrypted_keypair,
            raw_cert: self.data.certificate.to_der().to_owned(),
        };

        let encoded = postcard::to_extend(&on_disk, Vec::new())?;

        Ok(encoded)
    }

    pub async fn from_bytes(
        bytes: &[u8],
        password: Vec<u8>,
    ) -> Result<Self, DecodeCredentialsError> {
        let on_disk: CredentialOnDisk = postcard::from_bytes(bytes)?;

        let certificate = Certificate::from_der(on_disk.raw_cert)?;

        debug!("decrypting credentials with password");

        let decrypted_keypair =
            EncryptedData::decrypt_with_password(&on_disk.encrypted_keypair, password)
                .await
                .map_err(DecodeCredentialsError::DecryptError)?;

        debug!("credentials decrypted");

        Self::new(decrypted_keypair, certificate)
            .map_err(DecodeCredentialsError::CreateCredentialsError)
    }

    pub fn get_certificate(&self) -> &Certificate {
        &self.data.certificate
    }

    pub fn get_key_bytes(&self) -> &[u8] {
        &self.data.raw_keypair
    }

    pub fn approve_request(
        &self,
        request: CertificateRequest,
    ) -> Result<Certificate, ApproveRequestError> {
        let ca_keypair = rcgen::KeyPair::from_der(&self.data.raw_keypair)
            .map_err(ApproveRequestError::KeypairParseError)?;
        let ca_params =
            rcgen::CertificateParams::from_ca_cert_der(self.data.certificate.to_der(), ca_keypair)
                .map_err(ApproveRequestError::CreateCaParamsError)?;

        let ca = rcgen::Certificate::from_params(ca_params)
            .map_err(ApproveRequestError::CreateCaError)?;

        let new_cert_der = request
            .csr
            .serialize_der_with_signer(&ca)
            .map_err(ApproveRequestError::SignCertError)?;

        let new_cert =
            Certificate::from_der(new_cert_der).map_err(ApproveRequestError::ParseNewCertError)?;

        Ok(new_cert)
    }
}

impl CanSign for PermCredentials {
    fn borrow_keypair(&self) -> &Ed25519KeyPair {
        &self.data.keypair
    }
}

impl CanVerify for PermCredentials {
    fn borrow_public_key(&self) -> &[u8] {
        self.data.keypair.public_key().as_ref()
    }
}

#[cfg(test)]
mod test {
    use ring::rand::{SecureRandom, SystemRandom};

    use crate::{Keypair, PermCredentials};

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

        let on_disk = original.to_bytes(pw.clone().into()).await.unwrap();

        let copy = PermCredentials::from_bytes(&on_disk, pw.into())
            .await
            .unwrap();

        assert_eq!(copy.data.raw_keypair, original.data.raw_keypair);
        assert_eq!(copy.data.certificate, original.data.certificate);
    }
}
