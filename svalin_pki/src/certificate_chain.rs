use std::collections::HashSet;

use crate::{
    Certificate, CertificateType, SignatureVerificationError, ValidityError,
    certificate::{SpkiHash, UnverifiedCertificate},
};
use serde::{Deserialize, Serialize};

pub struct CertificateChainBuilder {
    certificates: Vec<UnverifiedCertificate>,
    known_spki_hashes: HashSet<String>,
}

#[derive(Serialize, Deserialize)]
pub struct UnverifiedCertificateChain {
    certificates: Vec<UnverifiedCertificate>,
}

pub struct CertificateChain {
    certificates: Vec<Certificate>,
}

#[derive(Debug, thiserror::Error)]
pub enum AddCertificateError {
    #[error("Signature loop detected. Certificates order is {0:?}")]
    SignatureLoop(Vec<UnverifiedCertificate>),
    #[error("Couldn't verify signature with given parent")]
    SignatureVerificationError(#[from] SignatureVerificationError),
    #[error("Parent with wrong spki_hash given. Expected {0}, got {1}")]
    WrongParent(String, String),
}

impl CertificateChainBuilder {
    pub fn new(leaf_certificate: UnverifiedCertificate) -> Self {
        let mut known_spki_hashes = HashSet::new();
        known_spki_hashes.insert(leaf_certificate.spki_hash().to_string());
        CertificateChainBuilder {
            known_spki_hashes,
            certificates: vec![leaf_certificate],
        }
    }

    pub fn push_parent(
        &mut self,
        parent: UnverifiedCertificate,
    ) -> Result<Option<UnverifiedCertificateChain>, AddCertificateError> {
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
            Ok(Some(UnverifiedCertificateChain {
                certificates: self.certificates.iter().rev().cloned().collect(),
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
    WrongRoot(Certificate, UnverifiedCertificate),
    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(#[from] SignatureVerificationError),
    #[error("Certificate validity error: {0}")]
    ValidityError(#[from] ValidityError),
}

impl UnverifiedCertificateChain {
    pub fn verify(
        self,
        root: &Certificate,
        timestamp: u64,
    ) -> Result<CertificateChain, VerifyChainError> {
        let chain_root = self
            .certificates
            .first()
            .ok_or(VerifyChainError::EmptyChain)?;

        if chain_root != root {
            return Err(VerifyChainError::WrongRoot(
                root.clone(),
                chain_root.clone(),
            ));
        }

        let mut verified_chain = Vec::with_capacity(self.certificates.len());
        verified_chain.push(root.clone());

        if self.certificates.len() == 1 {
            return Ok(CertificateChain {
                certificates: vec![root.clone()],
            });
        }

        for current in self.certificates.into_iter().skip(1) {
            let verified_cert = current.verify_signature(
                verified_chain
                    .last()
                    .expect("prefilled Vec cannot be empty"),
                timestamp,
            )?;
            verified_chain.push(verified_cert);
        }

        Ok(CertificateChain {
            certificates: verified_chain,
        })
    }
}
