use anyhow::Result;
use x509_parser::nom::AsBytes;
use x509_parser::{certificate::X509Certificate, oid_registry::asn1_rs::FromDer};

use crate::signed_message::CanVerify;

#[derive(Debug)]
pub struct Certificate {
    der: Vec<u8>,
    public_key: Vec<u8>,
}

impl Certificate {
    pub fn from_der(der: Vec<u8>) -> Result<Certificate> {
        let (_, cert) = X509Certificate::from_der(der.as_bytes())?;
        let public_key = cert.public_key().raw.to_owned();

        Ok(Certificate { der, public_key })
    }

    pub fn to_der(&self) -> &[u8] {
        &self.der
    }
}

impl PartialEq for Certificate {
    fn eq(&self, other: &Self) -> bool {
        self.der == other.der
    }
}

impl CanVerify for Certificate {
    fn borrow_public_key(&self) -> &[u8] {
        return self.public_key.as_ref();
    }
}

mod test {
    use ring::rand::{SecureRandom, SystemRandom};

    use crate::{
        signed_message::{Sign, Verify},
        TempCredentials,
    };

    #[test]
    pub fn cert_verify_message() {
        let credentials = TempCredentials::generate().unwrap();
        let rand = SystemRandom::new();

        let mut msg = [0u8; 1024];
        rand.fill(&mut msg).unwrap();

        let signed = credentials.sign(&msg).unwrap();

        let cert = credentials.to_self_signed_cert().unwrap();
        let msg2 = cert.verify(&signed).unwrap();

        assert_eq!(msg, msg2.as_ref());
    }
}
