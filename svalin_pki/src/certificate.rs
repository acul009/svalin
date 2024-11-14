use std::fmt::Debug;
use std::sync::Arc;

use anyhow::Result;
use ring::error;
use serde::de::Visitor;
use serde::{de, Deserialize, Serialize};
use spki::FingerprintBytes;
use thiserror::Error;
use x509_parser::error::X509Error;
use x509_parser::nom::{AsBytes, HexDisplay};
use x509_parser::prelude::Validity;
use x509_parser::{certificate::X509Certificate, oid_registry::asn1_rs::FromDer};

use crate::signed_message::CanVerify;

#[derive(Debug)]
struct CertificateData {
    der: Vec<u8>,
    public_key: Vec<u8>,
    spki_hash: String,
    issuer: String,
    validity: Validity,
}

#[derive(Debug, Clone)]
pub struct Certificate {
    data: Arc<CertificateData>,
}

#[derive(Error, Debug)]
pub enum CertificateParseError {
    #[error("X509 Parser Error: {0}")]
    X509ParserError(#[from] x509_parser::nom::Err<X509Error>),
    #[error("X509 Error: {0}")]
    X509Error(#[from] X509Error),
    #[error("Missing Common Name")]
    MissingCommonName,
    #[error("Issuer Missing Common Name")]
    IssuerMissingCommonName,
    #[error("SPKI Hash Mismatch - certificate is not following convention")]
    SpkiHashMismatch,
}

impl Certificate {
    pub fn from_der(der: Vec<u8>) -> Result<Certificate, CertificateParseError> {
        let (_, cert) = X509Certificate::from_der(der.as_bytes())?;

        let public_key = cert.public_key().subject_public_key.data.to_vec();

        let spki_hash = ring::digest::digest(&ring::digest::SHA512_256, &public_key)
            .as_ref()
            .to_hex(32);

        let cn = cert
            .subject()
            .iter_common_name()
            .next()
            .ok_or_else(|| CertificateParseError::MissingCommonName)?
            .as_str()?;

        if spki_hash != cn {
            return Err(CertificateParseError::SpkiHashMismatch);
        };

        let validity = cert.validity().clone();

        let issuer = cert
            .issuer()
            .iter_common_name()
            .next()
            .ok_or_else(|| CertificateParseError::MissingCommonName)?
            .as_str()?
            .to_string();

        Ok(Certificate {
            data: Arc::new(CertificateData {
                der,
                public_key,
                spki_hash,
                issuer,
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

    // pub fn get_fingerprint(&self) -> FingerprintBytes {
    //     let hash = ring::digest::digest(&ring::digest::SHA512_256,
    // &self.data.der);

    //     hash.as_ref()[0..32].try_into().unwrap()
    // }

    pub fn spki_hash(&self) -> &str {
        &self.data.spki_hash
    }

    pub fn fingerprint(&self) -> FingerprintBytes {
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

#[derive(Debug, Error)]
pub enum ValidityError {
    #[error("The certificate is not yet valid")]
    NotYetValid,
    #[error("The certificate is expired")]
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
