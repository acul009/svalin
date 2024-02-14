use crate::{
    signed_message::{CanSign, CanVerify},
    Certificate, PermCredentials,
};
use anyhow::{anyhow, Result};
use rcgen::{DnType, ExtendedKeyUsagePurpose, KeyUsagePurpose, SignatureAlgorithm};
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use time::{Duration, OffsetDateTime};

pub struct Keypair {
    keypair: Ed25519KeyPair,
    raw: Vec<u8>,
    alg: &'static SignatureAlgorithm,
}

impl Keypair {
    pub fn generate() -> Result<Keypair> {
        let rand = SystemRandom::new();
        let keypair_pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rand).unwrap();
        let raw = keypair_pkcs8.as_ref().to_owned();
        let keypair = ring::signature::Ed25519KeyPair::from_pkcs8(keypair_pkcs8.as_ref()).unwrap();

        Ok(Keypair {
            keypair,
            raw,
            alg: &rcgen::PKCS_ED25519,
        })
    }

    pub fn to_self_signed_cert(self) -> Result<PermCredentials> {
        let rc_keypair = rcgen::KeyPair::from_der(self.raw.as_ref())?;
        let mut params = rcgen::CertificateParams::new(vec![]);
        params.key_pair = Some(rc_keypair);
        params.alg = self.alg;
        params.not_before = OffsetDateTime::now_utc();
        params.not_after = OffsetDateTime::now_utc().saturating_add(Duration::days(365 * 10));
        params.key_usages.push(KeyUsagePurpose::DigitalSignature);
        params
            .extended_key_usages
            .push(ExtendedKeyUsagePurpose::ServerAuth);
        params.use_authority_key_identifier_extension = true;

        let mut uuid = vec![0u8; 128];
        uuid::Uuid::new_v4().as_hyphenated().encode_lower(&mut uuid);
        params
            .distinguished_name
            .push(DnType::CommonName, String::from_utf8(uuid)?);

        let ca_cert = rcgen::Certificate::from_params(params)?;
        let ca_der = ca_cert.serialize_der()?;

        let certificate = Certificate::from_der(ca_der)?;

        self.upgrade(certificate)
    }

    pub fn generate_request(&self) -> Result<String> {
        let rc_keypair = rcgen::KeyPair::from_der(self.raw.as_ref())?;
        let mut params = rcgen::CertificateParams::new(vec![]);
        params.key_pair = Some(rc_keypair);
        params.alg = self.alg;

        let mut uuid = vec![0u8; 128];
        uuid::Uuid::new_v4().as_hyphenated().encode_lower(&mut uuid);
        params
            .distinguished_name
            .push(DnType::CommonName, String::from_utf8(uuid)?);

        let temp_cert = rcgen::Certificate::from_params(params)?;
        Ok(temp_cert.serialize_request_pem()?)
    }

    pub fn upgrade(self, certificate: Certificate) -> Result<PermCredentials> {
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
        signed_message::{Sign, Verify},
        Keypair,
    };

    #[test]
    fn test_sign() {
        let credentials = Keypair::generate().unwrap();
        let rand = SystemRandom::new();

        let mut msg = [0u8; 1024];
        rand.fill(&mut msg).unwrap();

        let signed = credentials.sign(&msg).unwrap();

        let msg2 = credentials.verify(&signed).unwrap();

        assert_eq!(msg, msg2.as_ref());
    }

    #[test]
    fn tampered_sign() {
        let credentials = Keypair::generate().unwrap();
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
        let credentials = Keypair::generate().unwrap();
        credentials.to_self_signed_cert().unwrap();
    }
}
