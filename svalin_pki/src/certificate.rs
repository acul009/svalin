use std::fmt::Debug;
use std::sync::Arc;

use anyhow::Result;
use serde::de::Visitor;
use serde::{de, Deserialize, Serialize};
use spki::FingerprintBytes;
use x509_parser::nom::AsBytes;
use x509_parser::prelude::Validity;
use x509_parser::{certificate::X509Certificate, oid_registry::asn1_rs::FromDer};

use crate::signed_message::CanVerify;

#[derive(Debug)]
struct CertificateData {
    der: Vec<u8>,
    public_key: Vec<u8>,
    validity: Validity,
}

#[derive(Debug, Clone)]
pub struct Certificate {
    data: Arc<CertificateData>,
}

impl Certificate {
    pub fn from_der(der: Vec<u8>) -> Result<Certificate> {
        let (_, cert) = X509Certificate::from_der(der.as_bytes())?;

        let public_key = cert.public_key().subject_public_key.data.to_vec();

        let validity = cert.validity().clone();

        Ok(Certificate {
            data: Arc::new(CertificateData {
                der,
                public_key,
                validity,
            }),
        })
    }

    pub fn public_key(&self) -> &[u8] {
        &self.data.public_key
    }

    pub fn to_der(&self) -> &[u8] {
        &self.data.der
    }

    pub fn get_fingerprint(&self) -> FingerprintBytes {
        let hash = ring::digest::digest(&ring::digest::SHA512_256, &self.data.der);

        hash.as_ref()[0..32].try_into().unwrap()
    }

    pub fn check_validity_at(&self, time: u64) -> Result<(), ValidityError> {
        // Todo: maybe not try to panic here or at least verify that this conversion
        // always works
        if time
            < self
                .data
                .validity
                .not_before
                .timestamp()
                .try_into()
                .unwrap()
        {
            return Err(ValidityError::NotYetValid);
        } else if time > self.data.validity.not_after.timestamp().try_into().unwrap() {
            return Err(ValidityError::Expired);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum ValidityError {
    NotYetValid,
    Expired,
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
        deserializer.deserialize_byte_buf(CertificateVisitor)
    }
}

struct CertificateVisitor;

impl<'de> Visitor<'de> for CertificateVisitor {
    type Value = Certificate;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a der encoded certificate")
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Certificate::from_der(v).map_err(|err| de::Error::custom(err))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_byte_buf(v.to_vec())
    }
}

impl Ord for Certificate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.data.der.cmp(&other.data.der)
    }
}

impl PartialOrd for Certificate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.data.der.partial_cmp(&other.data.der)
    }
}

impl Eq for Certificate {}
