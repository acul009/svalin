use std::sync::Arc;

use anyhow::Result;
use serde::{de, Deserialize, Serialize};
use x509_parser::nom::AsBytes;
use x509_parser::{certificate::X509Certificate, oid_registry::asn1_rs::FromDer};
use zeroize::ZeroizeOnDrop;

use crate::signed_message::CanVerify;

#[derive(Debug, ZeroizeOnDrop)]
struct CertificateData {
    der: Vec<u8>,
    public_key: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Certificate {
    data: Arc<CertificateData>,
}

impl Certificate {
    pub fn from_der(der: Vec<u8>) -> Result<Certificate> {
        let (_, cert) = X509Certificate::from_der(der.as_bytes())?;
        let public_key = cert.public_key().raw.to_owned();

        Ok(Certificate {
            data: Arc::new(CertificateData { der, public_key }),
        })
    }

    pub fn public_key(&self) -> &[u8] {
        &self.data.public_key
    }

    pub fn to_der(&self) -> &[u8] {
        &self.data.der
    }
}

impl PartialEq for Certificate {
    fn eq(&self, other: &Self) -> bool {
        self.data.der == other.data.der
    }
}

impl CanVerify for Certificate {
    fn borrow_public_key(&self) -> &[u8] {
        return self.data.public_key.as_ref();
    }
}

impl Serialize for Certificate {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.to_der())
    }
}

impl<'de> Deserialize<'de> for Certificate {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let der = Vec::<u8>::deserialize(deserializer)?;
        Certificate::from_der(der).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod test {
    use ring::rand::{SecureRandom, SystemRandom};

    use crate::{
        signed_message::{Sign, Verify},
        Certificate, Keypair,
    };

    #[test]
    pub fn cert_verify_message() {
        let credentials = Keypair::generate().unwrap();
        let rand = SystemRandom::new();

        let mut msg = [0u8; 1024];
        rand.fill(&mut msg).unwrap();

        let signed = credentials.sign(&msg).unwrap();

        let cert = credentials.to_self_signed_cert().unwrap();
        let msg2 = cert.verify(&signed).unwrap();

        assert_eq!(msg, msg2.as_ref());
    }

    #[test]
    pub fn serialization() {
        let credentials = Keypair::generate().unwrap();
        let perm_creds = credentials.to_self_signed_cert().unwrap();
        let cert = perm_creds.get_certificate();

        let seriaized = cert.to_der().to_owned();
        let cert2 = Certificate::from_der(seriaized).unwrap();
        assert_eq!(cert, &cert2)
    }

    #[test]
    pub fn serde_serialization() {
        let credentials = Keypair::generate().unwrap();
        let perm_creds = credentials.to_self_signed_cert().unwrap();
        let cert = perm_creds.get_certificate();

        let serialized = postcard::to_extend(cert, Vec::new()).unwrap();

        let cert2: Certificate = postcard::from_bytes(&serialized).unwrap();
        assert_eq!(cert, &cert2)
    }
}
