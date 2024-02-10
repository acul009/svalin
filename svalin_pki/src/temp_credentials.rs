use crate::{
    signed_message::{CanSign, CanVerify},
    Certificate, PermCredentials,
};
use anyhow::Result;
use rcgen::{ExtendedKeyUsagePurpose, KeyUsagePurpose};
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use time::{Duration, OffsetDateTime};

pub struct TempCredentials {
    keypair: Ed25519KeyPair,
    raw: Vec<u8>,
}

impl TempCredentials {
    pub fn generate() -> Result<TempCredentials> {
        let rand = SystemRandom::new();
        let keypair_pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rand).unwrap();
        let raw = keypair_pkcs8.as_ref().to_owned();
        let keypair = ring::signature::Ed25519KeyPair::from_pkcs8(keypair_pkcs8.as_ref()).unwrap();

        Ok(TempCredentials {
            keypair: keypair,
            raw: raw,
        })
    }

    pub fn to_self_signed_cert(self) -> Result<PermCredentials> {
        let rc_keypair = rcgen::KeyPair::from_der(&self.raw.as_ref())?;
        let mut params = rcgen::CertificateParams::new(vec![]);
        params.key_pair = Some(rc_keypair);
        params.alg = &rcgen::PKCS_ED25519;
        params.not_before = OffsetDateTime::now_utc();
        params.not_after = OffsetDateTime::now_utc().saturating_add(Duration::days(365));
        params.key_usages.push(KeyUsagePurpose::DigitalSignature);
        params
            .extended_key_usages
            .push(ExtendedKeyUsagePurpose::ServerAuth);
        params.use_authority_key_identifier_extension = true;

        let ca_cert = rcgen::Certificate::from_params(params)?;
        let ca_der = ca_cert.serialize_der()?;

        let certificate = Certificate::from_der(ca_der)?;

        self.upgrade(certificate)
    }

    pub fn upgrade(self, certificate: Certificate) -> Result<PermCredentials> {
        PermCredentials::new(self.raw, certificate)
    }
}

impl CanSign for TempCredentials {
    fn borrow_keypair(&self) -> &Ed25519KeyPair {
        &self.keypair
    }
}

impl CanVerify for TempCredentials {
    fn borrow_public_key(&self) -> &[u8] {
        self.keypair.public_key().as_ref()
    }
}

#[cfg(test)]
mod test {

    use ring::rand::{SecureRandom, SystemRandom};

    use crate::{
        signed_message::{Sign, Verify},
        TempCredentials,
    };

    #[test]
    fn test_sign() {
        let credentials = TempCredentials::generate().unwrap();
        let rand = SystemRandom::new();

        let mut msg = [0u8; 1024];
        rand.fill(&mut msg).unwrap();

        let signed = credentials.sign(&msg).unwrap();

        let msg2 = credentials.verify(&signed).unwrap();

        assert_eq!(msg, msg2.as_ref());
    }

    #[test]
    fn tampered_sign() {
        let credentials = TempCredentials::generate().unwrap();
        let rand = SystemRandom::new();

        let mut msg = [0u8; 1024];
        rand.fill(&mut msg).unwrap();

        let mut signed = credentials.sign(&msg).unwrap();

        let replacement = &[1, 2, 3];
        signed.splice(50..52, replacement.iter().cloned());

        let msg2 = credentials.verify(&signed);
        match msg2 {
            Err(_) => assert!(true),
            Ok(_) => assert!(false, "message should not be readable after tampering"),
        }
    }

    #[test]
    fn create_self_signed() {
        let credentials = TempCredentials::generate().unwrap();
        credentials.to_self_signed_cert().unwrap();
    }
}
