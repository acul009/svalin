use std::collections::HashSet;

use crate::{
    Certificate, CertificateType, RootCertificate, SignatureVerificationError, ValidityError,
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
    _root: RootCertificate,
    certificates: Vec<Certificate>,
}

impl CertificateChain {
    pub fn take_leaf(mut self) -> Certificate {
        self.certificates
            .pop()
            .expect("A verified certificate chain cannot be empty")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AddCertificateError {
    #[error("Signature loop detected. Certificates order is {0:?}")]
    SignatureLoop(Vec<UnverifiedCertificate>),
    #[error("Couldn't verify signature with given parent")]
    SignatureVerificationError(#[from] SignatureVerificationError),
    #[error("Parent with wrong spki_hash given. Expected {0}, got {1}")]
    WrongParent(String, String),
    #[error("Certificate chain is already finished")]
    AlreadyFinished,
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
    ) -> Result<(), AddCertificateError> {
        let last_cert = self
            .certificates
            .last()
            .expect("constructor ensures no empty vec");

        if last_cert.certificate_type() == CertificateType::Root {
            return Err(AddCertificateError::AlreadyFinished);
        }

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

        self.certificates.push(parent);

        Ok(())
    }

    pub fn requested_issuer(&self) -> Option<&SpkiHash> {
        let last_cert = self
            .certificates
            .last()
            .expect("a certificate chain cannot be empty");

        if last_cert.certificate_type() == CertificateType::Root {
            None
        } else {
            Some(last_cert.issuer())
        }
    }

    pub fn is_finished(&self) -> bool {
        self.certificates
            .last()
            .expect("a certificate chain cannot be empty")
            .certificate_type()
            == CertificateType::Root
    }

    pub fn finish(self) -> Result<UnverifiedCertificateChain, ()> {
        if self.is_finished() {
            Ok(UnverifiedCertificateChain {
                certificates: self.certificates.iter().rev().cloned().collect(),
            })
        } else {
            Err(())
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyChainError {
    #[error("Empty certificate chain")]
    EmptyChain,
    #[error("Wrong root certificate: expected {0:?}, found {1:?}")]
    WrongRoot(RootCertificate, UnverifiedCertificate),
    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(#[from] SignatureVerificationError),
    #[error("Certificate validity error: {0}")]
    ValidityError(#[from] ValidityError),
}

impl UnverifiedCertificateChain {
    pub fn verify(
        self,
        root: &RootCertificate,
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

        if self.certificates.len() == 1 {
            return Ok(CertificateChain {
                _root: root.clone(),
                certificates: vec![],
            });
        }

        for current in self.certificates.into_iter().skip(1) {
            let parent = verified_chain
                .last()
                .or(Some(root.as_certificate()))
                .expect("prefilled Vec cannot be empty");

            let verified_cert = current.verify_signature(parent, timestamp)?;
            verified_chain.push(verified_cert);
        }

        Ok(CertificateChain {
            _root: root.clone(),
            certificates: verified_chain,
        })
    }
}
