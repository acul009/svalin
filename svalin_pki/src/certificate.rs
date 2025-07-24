use std::fmt::{Debug, Write};
use std::hash::Hash;
use std::sync::Arc;

use anyhow::Result;
use serde::de::Visitor;
use serde::{Deserialize, Serialize, de};
use thiserror::Error;
use x509_parser::error::X509Error;
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

#[derive(Clone)]
pub struct Certificate {
    data: Arc<CertificateData>,
}

impl Debug for Certificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Certificate")
            .field("fingerprint", &self.fingerprint())
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum ValidityError {
    #[error("The certificate is not yet valid")]
    NotYetValid,
    #[error("The certificate is expired")]
    Expired,
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

#[derive(Error, Debug)]
pub enum SignatureVerificationError {
    #[error("X509 Parser Error: {0}")]
    X509ParserError(#[from] x509_parser::nom::Err<X509Error>),
    #[error("Verification Error: {0}")]
    X509VerificationError(#[from] X509Error),
}

#[derive(PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Fingerprint([u8; 32]);

impl Debug for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0.iter() {
            write!(f, "{:02X}", byte)?
        }

        Ok(())
    }
}

impl Certificate {
    pub fn from_der(der: Vec<u8>) -> Result<Certificate, CertificateParseError> {
        let (_, cert) = X509Certificate::from_der(der.as_ref())?;

        let spki_hash = Self::compute_spki_hash(&cert.public_key().raw.to_vec());

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

        let public_key = cert.public_key().subject_public_key.as_ref().to_vec();

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

    pub(crate) fn compute_spki_hash(spki_der: &[u8]) -> String {
        let raw_identifier = ring::digest::digest(&ring::digest::SHA512_256, spki_der);

        let identifier = raw_identifier.as_ref().iter().fold(
            String::with_capacity(raw_identifier.as_ref().len() * 2),
            |mut string, byte| {
                write!(string, "{:02X}", byte)
                    .expect("Writing to a preallocated string is unlikely to fail");
                string
            },
        );

        identifier
    }

    pub fn public_key(&self) -> &[u8] {
        &self.data.public_key
    }

    pub fn to_der(&self) -> &[u8] {
        &self.data.der
    }

    pub fn issuer(&self) -> &str {
        &self.data.issuer
    }

    pub fn spki_hash(&self) -> &str {
        &self.data.spki_hash
    }

    pub fn fingerprint(&self) -> Fingerprint {
        // Todo: use rcgen::Certificate::key_identifier instead
        let hash = ring::digest::digest(&ring::digest::SHA512_256, &self.data.der);

        let fingerprint = hash.as_ref()[0..32].try_into().unwrap();
        Fingerprint(fingerprint)
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

    /// Verify the signature of the current certificate using the given issue certificate
    pub fn verify_signature(&self, issuer: &Certificate) -> Result<(), SignatureVerificationError> {
        let (_, cert) = X509Certificate::from_der(&self.data.der)?;

        let (_, issuer_cert) = X509Certificate::from_der(&issuer.data.der)?;

        cert.verify_signature(Some(issuer_cert.public_key()))?;

        Ok(())
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

    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_byte_buf(v.to_vec())
    }

    fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut der = Vec::new();
        while let Some(byte) = seq.next_element::<u8>()? {
            der.push(byte);
        }

        self.visit_byte_buf(der)
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

impl Hash for Certificate {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.data.der.hash(state);
    }
}
