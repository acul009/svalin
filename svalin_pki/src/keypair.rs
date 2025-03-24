use crate::{
    Certificate, CertificateParseError, PermCredentials,
    perm_credentials::CreateCredentialsError,
    signed_message::{CanSign, CanVerify},
};
use anyhow::Result;
use rcgen::{DnType, ExtendedKeyUsagePurpose, KeyUsagePurpose, SignatureAlgorithm};
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use time::{Duration, OffsetDateTime};
use x509_parser::nom::HexDisplay;

pub struct Keypair {
    keypair: Ed25519KeyPair,
    raw: Vec<u8>,
    alg: &'static SignatureAlgorithm,
}

#[derive(Debug, thiserror::Error)]
pub enum ToSelfSingedError {
    #[error("error parsing keypair: {0}")]
    ParseKeypairError(rcgen::Error),
    #[error("error creating certificate: {0}")]
    CreateCertError(rcgen::Error),
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
    #[error("error parsing certificate: {0}")]
    ParseCertificateError(rcgen::Error),
    #[error("error serializing keypair: {0}")]
    SerializeError(rcgen::Error),
}

impl Keypair {
    pub fn generate() -> Keypair {
        let rand = SystemRandom::new();
        let keypair_pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rand).unwrap();
        let raw = keypair_pkcs8.as_ref().to_owned();
        let keypair = ring::signature::Ed25519KeyPair::from_pkcs8(keypair_pkcs8.as_ref()).unwrap();

        Keypair {
            keypair,
            raw,
            alg: &rcgen::PKCS_ED25519,
        }
    }

    pub fn to_self_signed_cert(self) -> Result<PermCredentials, ToSelfSingedError> {
        let rc_keypair = rcgen::KeyPair::from_der(self.raw.as_ref())
            .map_err(ToSelfSingedError::ParseKeypairError)?;
        let mut params = rcgen::CertificateParams::new(vec![]);
        params.key_pair = Some(rc_keypair);
        params.alg = self.alg;
        params.not_before = OffsetDateTime::now_utc();
        params.not_after = OffsetDateTime::now_utc().saturating_add(Duration::days(365 * 10));
        params.key_usages.push(KeyUsagePurpose::DigitalSignature);
        params
            .extended_key_usages
            .push(ExtendedKeyUsagePurpose::ServerAuth);

        params
            .distinguished_name
            .push(DnType::CommonName, self.spki_hash());

        let ca_cert =
            rcgen::Certificate::from_params(params).map_err(ToSelfSingedError::CreateCertError)?;
        let ca_der = ca_cert
            .serialize_der()
            .map_err(ToSelfSingedError::SerializeError)?;

        let certificate =
            Certificate::from_der(ca_der).map_err(ToSelfSingedError::CertificateParseError)?;

        self.upgrade(certificate)
            .map_err(ToSelfSingedError::CreateCredentialsError)
    }

    pub fn generate_request(&self) -> Result<String, GenerateRequestError> {
        let rc_keypair = rcgen::KeyPair::from_der(self.raw.as_ref())
            .map_err(GenerateRequestError::ParseKeypairError)?;
        let mut params = rcgen::CertificateParams::new(vec![]);
        params.key_pair = Some(rc_keypair);
        params.alg = self.alg;

        params
            .distinguished_name
            .push(DnType::CommonName, self.spki_hash());

        let temp_cert = rcgen::Certificate::from_params(params)
            .map_err(GenerateRequestError::ParseCertificateError)?;
        Ok(temp_cert
            .serialize_request_pem()
            .map_err(GenerateRequestError::SerializeError)?)
    }

    /// This was recommended to me as a possible id by the rust crypto channel
    /// on discord
    pub fn spki_hash(&self) -> String {
        let spki_hash = ring::digest::digest(&ring::digest::SHA512_256, &self.public_key())
            .as_ref()
            .to_hex(32);
        spki_hash
    }

    pub fn upgrade(
        self,
        certificate: Certificate,
    ) -> Result<PermCredentials, CreateCredentialsError> {
        PermCredentials::new(self.raw, certificate)
    }

    pub fn public_key(&self) -> &[u8] {
        self.keypair.public_key().as_ref()
    }
}

impl CanSign for Keypair {
    fn borrow_keypair(&self) -> &Ed25519KeyPair {
        &self.keypair
    }
}

impl CanVerify for Keypair {
    fn borrow_public_key(&self) -> &[u8] {
        self.keypair.public_key().as_ref()
    }
}

#[cfg(test)]
mod test {

    use ring::rand::{SecureRandom, SystemRandom};

    use crate::{
        Keypair,
        signed_message::{Sign, Verify},
    };

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
