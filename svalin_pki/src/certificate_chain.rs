use std::collections::HashSet;

use crate::{
    Certificate, CertificateType, SignatureVerificationError, ValidityError, certificate::SpkiHash,
};
use serde::{Deserialize, Serialize};

pub struct CertificateChainBuilder {
    certificates: Vec<Certificate>,
    known_spki_hashes: HashSet<String>,
}

#[derive(Serialize, Deserialize)]
pub struct CertificateChain {
    certificates: Vec<Certificate>,
}

#[derive(Debug, thiserror::Error)]
pub enum AddCertificateError {
    #[error("Signature loop detected. Certificates order is {0:?}")]
    SignatureLoop(Vec<Certificate>),
    #[error("Couldn't verify signature with given parent")]
    SignatureVerificationError(#[from] SignatureVerificationError),
    #[error("Parent with wrong spki_hash given. Expected {0}, got {1}")]
    WrongParent(String, String),
}

impl CertificateChainBuilder {
    pub fn new(leaf_certificate: Certificate) -> Self {
        let mut known_spki_hashes = HashSet::new();
        known_spki_hashes.insert(leaf_certificate.spki_hash().to_string());
        CertificateChainBuilder {
            known_spki_hashes,
            certificates: vec![leaf_certificate],
        }
    }

    pub fn push_parent(
        &mut self,
        parent: Certificate,
    ) -> Result<Option<CertificateChain>, AddCertificateError> {
        let last_cert = self
            .certificates
            .last()
            .expect("constructor ensures no empty vec");

        if last_cert.issuer() != parent.spki_hash() {
            return Err(AddCertificateError::WrongParent(
                last_cert.spki_hash().to_string(),
                parent.spki_hash().to_string(),
            ));
        }

        last_cert.verify_signature(&parent)?;

        if !self
            .known_spki_hashes
            .insert(parent.spki_hash().to_string())
        {
            let fingerprint_order = self.certificates.iter().chain([&parent]).cloned().collect();
            return Err(AddCertificateError::SignatureLoop(fingerprint_order));
        }

        let parent_type = parent.certificate_type();
        self.certificates.push(parent);

        if parent_type == CertificateType::Root {
            Ok(Some(CertificateChain {
                certificates: self.certificates.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn requested_issuer(&self) -> Option<&SpkiHash> {
        let last_cert = self.certificates.last()?;

        if last_cert.certificate_type() == CertificateType::Root {
            None
        } else {
            Some(last_cert.issuer())
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyChainError {
    #[error("Empty certificate chain")]
    EmptyChain,
    #[error("Wrong root certificate: expected {0:?}, found {1:?}")]
    WrongRoot(Certificate, Certificate),
    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(#[from] SignatureVerificationError),
    #[error("Certificate validity error: {0}")]
    ValidityError(#[from] ValidityError),
}

impl CertificateChain {
    pub fn verify(
        &self,
        root: &Certificate,
        timestamp: u64,
    ) -> Result<&Certificate, VerifyChainError> {
        let chain_root = self
            .certificates
            .last()
            .ok_or(VerifyChainError::EmptyChain)?;

        if chain_root != root {
            return Err(VerifyChainError::WrongRoot(
                root.clone(),
                chain_root.clone(),
            ));
        }

        chain_root.check_validity_at(timestamp)?;

        if self.certificates.len() == 1 {
            return Ok(&self.certificates[0]);
        }

        let current_certificate = self.certificates.iter();
        let parent_certificate = self.certificates.iter().skip(1);

        for (current, parent) in current_certificate.zip(parent_certificate) {
            current.verify_signature(parent)?;
            current.check_validity_at(timestamp)?;
        }

        Ok(&self.certificates[0])
    }
}
