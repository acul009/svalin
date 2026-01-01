use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use rustls::pki_types::CertificateDer;
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
    Temporary,
    User,
    UserDevice,
    Agent,
    Server,
}

#[derive(Debug)]
pub struct CertificateData {
    der: Vec<u8>,
    certificate_type: CertificateType,
    public_key: Vec<u8>,
    spki_hash: SpkiHash,
    is_ca: bool,
    issuer: SpkiHash,
    validity: Validity,
}

#[derive(Clone)]
pub struct UnverifiedCertificate {
    data: Arc<CertificateData>,
}

impl Deref for UnverifiedCertificate {
    type Target = CertificateData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl Debug for UnverifiedCertificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnverifiedCertificate")
            .field("spki_hash", &self.spki_hash())
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct Certificate(UnverifiedCertificate);

impl Debug for Certificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Certificate")
            .field("spki_hash", &self.spki_hash())
            .finish()
    }
}

impl AsRef<Certificate> for &Certificate {
    fn as_ref(&self) -> &Certificate {
        self
    }
}

impl Deref for Certificate {
    type Target = CertificateData;

    fn deref(&self) -> &Self::Target {
        &self.0.data
    }
}

#[derive(Clone)]
pub struct RootCertificate(Certificate);
impl RootCertificate {
    pub fn as_certificate(&self) -> &Certificate {
        &self.0
    }

    pub fn to_certificate(self) -> Certificate {
        self.0
    }

    pub fn as_unverified(&self) -> &UnverifiedCertificate {
        &self.0.as_unverified()
    }

    pub fn to_unverified(self) -> UnverifiedCertificate {
        self.0.to_unverified()
    }
}

impl AsRef<Certificate> for &RootCertificate {
    fn as_ref(&self) -> &Certificate {
        self.as_certificate()
    }
}

impl Debug for RootCertificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RootCertificate")
            .field("spki_hash", &self.spki_hash())
            .finish()
    }
}

impl Deref for RootCertificate {
    type Target = CertificateData;

    fn deref(&self) -> &Self::Target {
        &self.0.0.data
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
    #[error("Self-signed certificate is not of type root or temporary")]
    SelfSignedNotRootOrTemp,
    #[error("Certificate validity is broken")]
    BrokenValidity,
    #[error("Incorrect validity duration for certificate type {0}: expected {1}, got {2}")]
    IncorrectValidityDuration(CertificateType, Duration, Duration),
    #[error("invalid issuer format: {0}")]
    InvalidIssuer(String),
}

#[derive(Error, Debug)]
pub enum SignatureVerificationError {
    #[error("X509 Parser Error: {0}")]
    X509ParserError(#[from] x509_parser::nom::Err<X509Error>),
    #[error("Verification Error: {0}")]
    X509VerificationError(#[from] X509Error),
    #[error("Certificate of type {0} is not allowed to sign certificates of type {1}")]
    InvalidCertificateType(CertificateType, CertificateType),
    #[error("Certificate validity could not be verified: {0}")]
    ValidityError(#[from] ValidityError),
}

#[derive(PartialEq, Eq, Clone, Serialize, Deserialize, PartialOrd, Ord, Hash)]
pub struct SpkiHash([u8; 32]);
impl Display for SpkiHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0.iter() {
            write!(f, "{:02X}", byte)?
        }

        Ok(())
    }
}

impl Debug for SpkiHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0.iter() {
            write!(f, "{:02X}", byte)?
        }

        Ok(())
    }
}

impl SpkiHash {
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UseAsRootError {
    #[error("certificate is not a root certificate")]
    InvalidCertificateType,
}

impl UnverifiedCertificate {
    pub fn from_der(der: Vec<u8>) -> Result<Self, CertificateParseError> {
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

        if spki_hash.to_string().as_str() != cn {
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

        let issuer_str = cert
            .issuer()
            .iter_common_name()
            .next()
            .ok_or_else(|| CertificateParseError::MissingCommonName)?
            .as_str()?;

        let mut issuer_data = [0u8; 32];
        if issuer_str.len() != 64 {
            return Err(CertificateParseError::InvalidIssuer(issuer_str.to_string()));
        }
        for i in 0..32 {
            let hex_byte = &issuer_str[i * 2..i * 2 + 2];
            issuer_data[i] = u8::from_str_radix(hex_byte, 16)
                .map_err(|_| CertificateParseError::InvalidIssuer(issuer_str.to_string()))?;
        }
        let issuer = SpkiHash(issuer_data);

        if cert.issuer() == cert.subject() {
            if certificate_type != CertificateType::Root
                && certificate_type != CertificateType::Temporary
            {
                return Err(CertificateParseError::SelfSignedNotRootOrTemp);
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

        Ok(Self {
            data: Arc::new(CertificateData {
                der,
                certificate_type,
                public_key,
                spki_hash,
                is_ca,
                issuer,
                validity,
            }),
        })
    }

    pub(crate) fn compute_spki_hash(spki_der: &[u8]) -> SpkiHash {
        let raw_identifier = ring::digest::digest(&ring::digest::SHA512_256, spki_der);
        let slice = raw_identifier.as_ref();
        SpkiHash(slice.try_into().unwrap())
    }

    /// Verify the signature of the current certificate using the given issue certificate
    pub fn verify_signature(
        self,
        issuer: impl AsRef<Certificate>,
        time: u64,
    ) -> Result<Certificate, SignatureVerificationError> {
        let issuer = issuer.as_ref();
        if !issuer
            .certificate_type()
            .may_be_parent_of(self.certificate_type())
        {
            return Err(SignatureVerificationError::InvalidCertificateType(
                issuer.certificate_type(),
                self.certificate_type(),
            ));
        }

        let (_, cert) = X509Certificate::from_der(&self.as_der())?;

        let (_, issuer_cert) = X509Certificate::from_der(&issuer.as_der())?;

        cert.verify_signature(Some(issuer_cert.public_key()))?;
        self.check_validity_at(time)?;

        Ok(Certificate(self))
    }

    pub(crate) fn mark_as_trusted(self) -> Certificate {
        Certificate(self)
    }

    pub fn use_as_temporary(self) -> Option<Certificate> {
        if self.data.certificate_type != CertificateType::Temporary {
            None
        } else {
            Some(Certificate(self))
        }
    }

    pub fn use_as_root(self) -> Result<RootCertificate, UseAsRootError> {
        if self.data.certificate_type == CertificateType::Root {
            Ok(RootCertificate(Certificate(self)))
        } else {
            Err(UseAsRootError::InvalidCertificateType)
        }
    }
}

impl CertificateData {
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    pub fn as_der(&self) -> &[u8] {
        &self.der
    }

    pub fn issuer(&self) -> &SpkiHash {
        &self.issuer
    }

    pub fn spki_hash(&self) -> &SpkiHash {
        &self.spki_hash
    }

    pub fn is_ca(&self) -> bool {
        self.is_ca
    }

    pub fn certificate_type(&self) -> CertificateType {
        self.certificate_type
    }

    pub fn check_validity_at(&self, time: u64) -> Result<(), ValidityError> {
        // Todo: maybe not try to panic here or at least verify that this conversion
        // always works
        if time < self.validity.not_before.timestamp().try_into().unwrap() {
            return Err(ValidityError::NotYetValid);
        } else if time > self.validity.not_after.timestamp().try_into().unwrap() {
            return Err(ValidityError::Expired);
        }

        Ok(())
    }
}

impl Certificate {
    pub fn as_unverified(&self) -> &UnverifiedCertificate {
        &self.0
    }

    pub fn to_unverified(self) -> UnverifiedCertificate {
        self.0
    }

    pub fn dangerous_from_already_verified_der(
        der: &CertificateDer<'_>,
    ) -> Result<Self, CertificateParseError> {
        Ok(UnverifiedCertificate::from_der(der.to_vec())?.mark_as_trusted())
    }
}

impl PartialEq for UnverifiedCertificate {
    fn eq(&self, other: &Self) -> bool {
        self.as_der() == other.as_der()
    }
}

impl PartialEq<Certificate> for UnverifiedCertificate {
    fn eq(&self, other: &Certificate) -> bool {
        self.as_der() == other.as_der()
    }
}

impl PartialEq<UnverifiedCertificate> for Certificate {
    fn eq(&self, other: &UnverifiedCertificate) -> bool {
        self.as_der() == other.as_der()
    }
}

impl PartialEq<RootCertificate> for UnverifiedCertificate {
    fn eq(&self, other: &RootCertificate) -> bool {
        self.as_der() == other.as_der()
    }
}

impl PartialEq<UnverifiedCertificate> for RootCertificate {
    fn eq(&self, other: &UnverifiedCertificate) -> bool {
        self.as_der() == other.as_der()
    }
}

impl PartialEq<Certificate> for RootCertificate {
    fn eq(&self, other: &Certificate) -> bool {
        self.as_der() == other.as_der()
    }
}

impl PartialEq<RootCertificate> for Certificate {
    fn eq(&self, other: &RootCertificate) -> bool {
        self.as_der() == other.as_der()
    }
}

impl CanVerify for Certificate {
    fn borrow_public_key(&self) -> &[u8] {
        return self.0.public_key.as_ref();
    }
}

impl Serialize for UnverifiedCertificate {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.as_der())
    }
}

impl<'de> Deserialize<'de> for UnverifiedCertificate {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_byte_buf(CertificateVisitor)
    }
}

struct CertificateVisitor;

impl<'de> Visitor<'de> for CertificateVisitor {
    type Value = UnverifiedCertificate;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a der encoded certificate")
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        UnverifiedCertificate::from_der(v).map_err(|err| de::Error::custom(err))
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

impl tls_codec::Size for UnverifiedCertificate {
    fn tls_serialized_len(&self) -> usize {
        self.as_der().len()
    }
}

impl tls_codec::Serialize for UnverifiedCertificate {
    fn tls_serialize<W: std::io::Write>(
        &self,
        writer: &mut W,
    ) -> std::result::Result<usize, tls_codec::Error> {
        writer
            .write(self.as_der())
            .map_err(|err| tls_codec::Error::EncodingError(err.to_string()))
    }
}

impl tls_codec::Deserialize for UnverifiedCertificate {
    fn tls_deserialize<R: std::io::Read>(
        bytes: &mut R,
    ) -> std::result::Result<Self, tls_codec::Error>
    where
        Self: Sized,
    {
        let mut buffer = Vec::new();
        bytes.read_to_end(&mut buffer)?;
        UnverifiedCertificate::from_der(buffer)
            .map_err(|err| tls_codec::Error::DecodingError(err.to_string()))
    }
}

impl Ord for Certificate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_der().cmp(&other.as_der())
    }
}

impl PartialOrd for Certificate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_der().partial_cmp(other.as_der())
    }
}

impl Eq for Certificate {}

impl Hash for Certificate {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_der().hash(state);
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
            CertificateType::Temporary => false,
        }
    }

    pub(crate) fn validity_duration(&self) -> Duration {
        match self {
            CertificateType::Root => Duration::days(365 * 10),
            CertificateType::User => Duration::days(365 * 1),
            CertificateType::UserDevice => Duration::days(30),
            CertificateType::Agent => Duration::days(365 * 1),
            CertificateType::Server => Duration::days(365 * 10),
            CertificateType::Temporary => Duration::minutes(1),
        }
    }

    fn may_be_parent_of(&self, child_type: CertificateType) -> bool {
        match self {
            CertificateType::Root => match child_type {
                CertificateType::Root => false,
                CertificateType::Server => true,
                CertificateType::User => true,
                CertificateType::Agent => true,
                CertificateType::UserDevice => true,
                CertificateType::Temporary => false,
            },
            CertificateType::User => match child_type {
                CertificateType::Root => false,
                CertificateType::Server => false,
                CertificateType::User => true,
                CertificateType::Agent => true,
                CertificateType::UserDevice => true,
                CertificateType::Temporary => false,
            },
            CertificateType::UserDevice => false,
            CertificateType::Agent => false,
            CertificateType::Server => false,
            CertificateType::Temporary => false,
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
            CertificateType::Temporary => write!(f, "temporary"),
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
            "temporary" => Ok(CertificateType::Temporary),
            _ => Err(CertificateTypeError::InvalidType),
        }
    }
}
