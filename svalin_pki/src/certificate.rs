use std::fmt::{Debug, Write};
use std::hash::Hash;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use serde::de::Visitor;
use serde::{Deserialize, Serialize, de};
use thiserror::Error;
use time::Duration;
use x509_parser::error::X509Error;
use x509_parser::prelude::Validity;
use x509_parser::{certificate::X509Certificate, oid_registry::asn1_rs::FromDer};

use crate::signed_message::CanVerify;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateType {
    Root,
    User,
    UserDevice,
    Agent,
    Server,
}

#[derive(Debug)]
struct CertificateData {
    der: Vec<u8>,
    certificate_type: CertificateType,
    public_key: Vec<u8>,
    spki_hash: String,
    is_ca: bool,
    issuer: String,
    validity: Validity,
    fingerprint: Fingerprint,
}

#[derive(Clone)]
pub struct Certificate {
    data: Arc<CertificateData>,
}

impl Debug for Certificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Certificate")
            .field("spki_hash", &self.spki_hash())
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
    #[error("Missing Organizational Unit")]
    MissingOrganizationalUnit,
    #[error("Invalid Certificate Type: {0}")]
    InvalidCertificateType(#[from] CertificateTypeError),
    #[error("Issuer Missing Common Name")]
    IssuerMissingCommonName,
    #[error("SPKI Hash Mismatch - certificate is not following convention")]
    SpkiHashMismatch,
    #[error("Certificate of type {0} should not be a CA")]
    ShouldNotBeCa(CertificateType),
    #[error("Certificate of type {0} should be a CA")]
    ShouldBeCa(CertificateType),
    #[error("Self-signed certificate is not of type root")]
    SelfSignedNotRoot,
    #[error("Certificate validity is broken")]
    BrokenValidity,
    #[error("Incorrect validity duration for certificate type {0}: expected {1}, got {2}")]
    IncorrectValidityDuration(CertificateType, Duration, Duration),
}

#[derive(Error, Debug)]
pub enum SignatureVerificationError {
    #[error("X509 Parser Error: {0}")]
    X509ParserError(#[from] x509_parser::nom::Err<X509Error>),
    #[error("Verification Error: {0}")]
    X509VerificationError(#[from] X509Error),
}

#[derive(PartialEq, Eq, Clone, Serialize, Deserialize, PartialOrd, Ord, Hash)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

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

        let certificate_type = CertificateType::from_str(
            cert.subject()
                .iter_organizational_unit()
                .next()
                .ok_or_else(|| CertificateParseError::MissingOrganizationalUnit)?
                .as_str()?,
        )?;

        if spki_hash != cn {
            return Err(CertificateParseError::SpkiHashMismatch);
        };

        let validity = cert.validity().clone();

        let validity_time = (validity.not_after - validity.not_before)
            .ok_or_else(|| CertificateParseError::BrokenValidity)?;

        let expected_validity = certificate_type.validity_duration();

        if validity_time != expected_validity {
            return Err(CertificateParseError::IncorrectValidityDuration(
                certificate_type,
                expected_validity,
                validity_time,
            ));
        }

        let issuer = cert
            .issuer()
            .iter_common_name()
            .next()
            .ok_or_else(|| CertificateParseError::MissingCommonName)?
            .as_str()?
            .to_string();

        if cert.issuer() == cert.subject() {
            if certificate_type != CertificateType::Root {
                return Err(CertificateParseError::SelfSignedNotRoot);
            }
        }

        let public_key = cert.public_key().subject_public_key.as_ref().to_vec();

        let is_ca = cert.is_ca();

        if is_ca != certificate_type.should_be_ca() {
            if is_ca {
                return Err(CertificateParseError::ShouldNotBeCa(certificate_type));
            } else {
                return Err(CertificateParseError::ShouldBeCa(certificate_type));
            }
        }

        // Todo: use rcgen::Certificate::key_identifier instead
        let hash = ring::digest::digest(&ring::digest::SHA512_256, &der);

        let fingerprint = Fingerprint(hash.as_ref()[0..32].try_into().unwrap());

        Ok(Certificate {
            data: Arc::new(CertificateData {
                der,
                certificate_type,
                public_key,
                spki_hash,
                is_ca,
                issuer,
                validity,
                fingerprint,
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

    pub fn is_ca(&self) -> bool {
        self.data.is_ca
    }

    pub fn fingerprint(&self) -> &Fingerprint {
        &self.data.fingerprint
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

    pub fn certificate_type(&self) -> CertificateType {
        self.data.certificate_type
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

impl tls_codec::Size for Certificate {
    fn tls_serialized_len(&self) -> usize {
        self.to_der().len()
    }
}

impl tls_codec::Serialize for Certificate {
    fn tls_serialize<W: std::io::Write>(
        &self,
        writer: &mut W,
    ) -> std::result::Result<usize, tls_codec::Error> {
        writer
            .write(self.to_der())
            .map_err(|err| tls_codec::Error::EncodingError(err.to_string()))
    }
}

impl tls_codec::Deserialize for Certificate {
    fn tls_deserialize<R: std::io::Read>(
        bytes: &mut R,
    ) -> std::result::Result<Self, tls_codec::Error>
    where
        Self: Sized,
    {
        let mut buffer = Vec::new();
        bytes.read_to_end(&mut buffer)?;
        Certificate::from_der(buffer)
            .map_err(|err| tls_codec::Error::DecodingError(err.to_string()))
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

impl CertificateType {
    pub fn should_be_ca(&self) -> bool {
        match self {
            CertificateType::Root => true,
            CertificateType::User => true,
            CertificateType::UserDevice => false,
            CertificateType::Agent => false,
            CertificateType::Server => false,
        }
    }

    pub fn validity_duration(&self) -> Duration {
        match self {
            CertificateType::Root => Duration::days(365 * 10),
            CertificateType::User => Duration::days(365 * 1),
            CertificateType::UserDevice => Duration::days(30),
            CertificateType::Agent => Duration::days(365 * 1),
            CertificateType::Server => Duration::days(365 * 10),
        }
    }
}

impl std::fmt::Display for CertificateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CertificateType::Root => write!(f, "root"),
            CertificateType::User => write!(f, "user"),
            CertificateType::UserDevice => write!(f, "user_device"),
            CertificateType::Agent => write!(f, "agent"),
            CertificateType::Server => write!(f, "server"),
        }
    }
}

#[derive(Debug, Error)]
pub enum CertificateTypeError {
    #[error("Invalid type")]
    InvalidType,
}

impl FromStr for CertificateType {
    type Err = CertificateTypeError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "root" => Ok(CertificateType::Root),
            "user" => Ok(CertificateType::User),
            "user_device" => Ok(CertificateType::UserDevice),
            "agent" => Ok(CertificateType::Agent),
            "server" => Ok(CertificateType::Server),
            _ => Err(CertificateTypeError::InvalidType),
        }
    }
}
