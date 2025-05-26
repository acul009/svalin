use std::fmt::Debug;

use crate::{
    Certificate, CertificateParseError, PermCredentials,
    encrypt::EncryptedObject,
    perm_credentials::CreateCredentialsError,
    signed_message::{CanSign, CanVerify},
};
use anyhow::Result;
use rcgen::{DnType, ExtendedKeyUsagePurpose, KeyUsagePurpose, SignatureAlgorithm};
use ring::signature::Ed25519KeyPair;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use x509_parser::nom::{AsBytes, HexDisplay};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub struct Keypair {
    keypair: rcgen::KeyPair,
    sign_keypair: Ed25519KeyPair,
    alg: Algorithm,
}

impl Debug for Keypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keypair").finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ToSelfSingedError {
    #[error("error parsing keypair: {0}")]
    ParseKeypairError(rcgen::Error),
    #[error("error creating params: {0}")]
    CreateParamsError(rcgen::Error),
    #[error("error self-signing certificate: {0}")]
    SelfSignError(rcgen::Error),
    #[error("error serializing keypair: {0}")]
    SerializeError(rcgen::Error),
    #[error("error parsing certificate: {0}")]
    CertificateParseError(CertificateParseError),
    #[error("error creating credentials: {0}")]
    CreateCredentialsError(CreateCredentialsError),
}

#[derive(Debug, thiserror::Error)]
pub enum GenerateRequestError {
    #[error("error parsing keypair: {0}")]
    ParseKeypairError(rcgen::Error),
    #[error("error creating params: {0}")]
    CreateParamsError(rcgen::Error),
    #[error("error serializing keypair: {0}")]
    SerializeError(rcgen::Error),
    #[error("error encoding request: {0}")]
    EncodeError(rcgen::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeKeypairError {
    #[error("error decrypting credentials: {0}")]
    DecryptError(#[from] crate::encrypt::DecryptError),
    #[error("error decoding credentials: {0}")]
    DecodeError(#[from] postcard::Error),
    #[error("error detecting key encoding")]
    DetectEncodingError,
    #[error("error parsing keypair: {0}")]
    ParseKeypairError(rcgen::Error),
}

#[derive(Serialize, Deserialize, Default, Clone, Zeroize)]
pub enum Algorithm {
    #[default]
    PkcsEd25519,
}

impl Algorithm {
    pub fn as_rcgen(&self) -> &'static SignatureAlgorithm {
        match self {
            Algorithm::PkcsEd25519 => &rcgen::PKCS_ED25519,
        }
    }
}

#[derive(Serialize, Deserialize, ZeroizeOnDrop)]
pub struct EncryptedKeypair {
    der: Vec<u8>,
    alg: Algorithm,
}

impl Keypair {
    pub fn generate() -> Self {
        let alg = Algorithm::default();
        let keypair = rcgen::KeyPair::generate_for(alg.as_rcgen()).unwrap();
        let sign_keypair = Ed25519KeyPair::from_pkcs8(keypair.serialized_der()).unwrap();

        Self {
            keypair,
            alg,
            sign_keypair,
        }
    }

    pub fn to_self_signed_cert(self) -> Result<PermCredentials, ToSelfSingedError> {
        let mut params =
            rcgen::CertificateParams::new(vec![]).map_err(ToSelfSingedError::CreateParamsError)?;
        params.not_before = OffsetDateTime::now_utc();
        params.not_after = OffsetDateTime::now_utc().saturating_add(Duration::days(365 * 10));
        params.key_usages.push(KeyUsagePurpose::DigitalSignature);
        params
            .extended_key_usages
            .push(ExtendedKeyUsagePurpose::ServerAuth);

        params
            .distinguished_name
            .push(DnType::CommonName, self.spki_hash());

        let ca_cert = params
            .self_signed(&self.keypair)
            .map_err(ToSelfSingedError::SelfSignError)?;

        let certificate = Certificate::from_der(ca_cert.der().to_vec())
            .map_err(ToSelfSingedError::CertificateParseError)?;

        self.upgrade(certificate)
            .map_err(ToSelfSingedError::CreateCredentialsError)
    }

    pub fn generate_request(&self) -> Result<String, GenerateRequestError> {
        let mut params = rcgen::CertificateParams::new(vec![])
            .map_err(GenerateRequestError::CreateParamsError)?;

        params
            .distinguished_name
            .push(DnType::CommonName, self.spki_hash());

        let request = params
            .serialize_request(&self.keypair)
            .map_err(GenerateRequestError::SerializeError)?;

        request.pem().map_err(GenerateRequestError::EncodeError)
    }

    /// This was recommended to me as a possible id by the rust crypto channel
    /// on discord
    pub fn spki_hash(&self) -> String {
        let spki_hash = ring::digest::digest(&ring::digest::SHA512_256, &self.public_key_der())
            .as_ref()
            .to_hex(32);
        spki_hash
    }

    pub fn upgrade(
        self,
        certificate: Certificate,
    ) -> Result<PermCredentials, CreateCredentialsError> {
        PermCredentials::new(self, certificate)
    }

    pub fn get_der_key_bytes(&self) -> &[u8] {
        self.keypair.serialized_der()
    }

    pub fn public_key_der(&self) -> &[u8] {
        self.keypair.public_key_raw()
    }

    pub async fn encrypt(&self, password: Vec<u8>) -> Result<EncryptedObject<EncryptedKeypair>> {
        let saved = EncryptedKeypair {
            der: self.keypair.serialize_der(),
            alg: self.alg.clone(),
        };

        Ok(EncryptedObject::encrypt_with_password(&saved, password).await?)
    }

    pub async fn decrypt(
        ciphertext: EncryptedObject<EncryptedKeypair>,
        password: Vec<u8>,
    ) -> Result<Self, DecodeKeypairError> {
        let saved = ciphertext.decrypt_with_password(password).await?;

        let keypair = rcgen::KeyPair::from_der_and_sign_algo(
            &saved
                .der
                .as_bytes()
                .try_into()
                .map_err(|_err| DecodeKeypairError::DetectEncodingError)?,
            saved.alg.as_rcgen(),
        )
        .map_err(DecodeKeypairError::ParseKeypairError)?;

        let sign_keypair = Ed25519KeyPair::from_pkcs8(keypair.serialized_der()).unwrap();

        Ok(Self {
            keypair,
            alg: saved.alg.clone(),
            sign_keypair,
        })
    }

    pub fn rcgen(&self) -> &rcgen::KeyPair {
        &self.keypair
    }

    pub fn alg(&self) -> &Algorithm {
        &self.alg
    }
}

impl CanSign for Keypair {
    fn borrow_keypair(&self) -> &Ed25519KeyPair {
        &self.sign_keypair
    }
}

impl CanVerify for Keypair {
    fn borrow_public_key(&self) -> &[u8] {
        self.public_key_der()
    }
}

#[cfg(test)]
mod test {

    use ring::rand::{SecureRandom, SystemRandom};

    use crate::{
        Keypair,
        signed_message::{Sign, Verify},
    };

    #[tokio::test]
    async fn test_encode_decode() {
        let credentials = Keypair::generate();
        let password = "testpass".as_bytes().to_owned();

        let encrypted = credentials.encrypt(password.clone()).await.unwrap();
        let _credentials2 = Keypair::decrypt(encrypted, password).await.unwrap();
    }

    #[test]
    fn test_sign() {
        let credentials = Keypair::generate();
        let rand = SystemRandom::new();

        let mut msg = [0u8; 1024];
        rand.fill(&mut msg).unwrap();

        let signed = credentials.sign(&msg).unwrap();

        let msg2 = credentials.verify(&signed).unwrap();

        assert_eq!(msg, msg2.as_ref());
    }

    #[test]
    fn tampered_sign() {
        let credentials = Keypair::generate();
        let rand = SystemRandom::new();

        let mut msg = [0u8; 1024];
        rand.fill(&mut msg).unwrap();

        let mut signed = credentials.sign(&msg).unwrap();

        let replacement = &[1, 2, 3];
        signed.splice(50..52, replacement.iter().cloned());

        let msg2 = credentials.verify(&signed);
        match msg2 {
            Err(_) => (),
            Ok(_) => panic!("message should not be readable after tampering"),
        }
    }

    #[test]
    fn create_self_signed() {
        let credentials = Keypair::generate();
        credentials.to_self_signed_cert().unwrap();
    }
}
