use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    AddCertificateError, Certificate, CertificateChainBuilder, Credential, RootCertificate,
    SignatureVerificationError, SpkiHash, UnverifiedCertificate, UnverifiedCertificateChain,
    UseAsRootError,
    secure_chain::{self, Chain, ChainDigest, ChainState, CheckedBlock, UncheckedBlock},
};
pub type CreateBlockError = secure_chain::CreateBlockError<Error>;
pub type CheckBlockError = secure_chain::CheckBlockError<Error>;

pub struct TrustStore {
    chain: Chain<State>,
}

impl fmt::Debug for TrustStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrustStore").finish()
    }
}

#[derive(Serialize, Debug)]
#[serde(transparent)]
pub struct TrustStoreDigest(secure_chain::ChainDigest);

impl TrustStore {
    pub fn initialize(root: RootCertificate) -> Self {
        let mut certificates = HashMap::new();
        certificates.insert(root.spki_hash().clone(), root.as_certificate().clone());
        let state = State { root, certificates };

        Self {
            chain: Chain::initialize(state),
        }
    }

    pub fn check(
        &mut self,
        block: UncheckedBlock<Transaction>,
    ) -> Result<CheckedBlock<Transaction>, CheckBlockError> {
        let certificate = self
            .chain
            .state()
            .certificates
            .get(block.signer())
            .ok_or(CheckBlockError::InvalidTransaction(Error::SignerNotKnown))?
            .clone();

        self.chain.check(block, &certificate)
    }

    pub fn apply(&mut self, block: CheckedBlock<Transaction>) {
        self.chain.apply(block)
    }

    pub fn add(
        &mut self,
        certificate: Certificate,
        credential: &Credential,
    ) -> Result<CheckedBlock<Transaction>, CreateBlockError> {
        self.chain
            .package(Transaction::Add(certificate.to_unverified()), credential)
    }

    pub fn export(&self) -> Exported {
        Exported {
            chain: self.chain.export(),
        }
    }

    pub fn import(exported: Exported) -> Result<Self, secure_chain::ImportError<ImportError>> {
        Ok(Self {
            chain: secure_chain::Chain::import(exported.chain)?,
        })
    }

    pub fn get(&self, spki_hash: &SpkiHash) -> Option<&Certificate> {
        self.chain.state().certificates.get(&spki_hash)
    }

    pub fn root(&self) -> &RootCertificate {
        &self.chain.state().root
    }

    pub fn complete_certificate_chain(
        &self,
        mut cert_chain: CertificateChainBuilder,
    ) -> Result<UnverifiedCertificateChain, CompleteCertChainError> {
        while let Some(issuer_spki) = cert_chain.requested_issuer() {
            let Some(issuer) = self.get(&issuer_spki) else {
                return Err(CompleteCertChainError::NotFound(issuer_spki.clone()));
            };

            cert_chain.push_parent(issuer.clone().to_unverified())?;
        }

        Ok(cert_chain
            .finish()
            .expect("already checked if the chain is finished"))
    }

    pub fn sequence(&self) -> u64 {
        self.chain.sequence()
    }

    pub fn digest(&self) -> ChainDigest {
        self.chain.digest()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Exported {
    chain: secure_chain::ExportedChain<State>,
}

struct State {
    root: RootCertificate,
    certificates: HashMap<SpkiHash, Certificate>,
}

impl State {
    pub fn initialize(root: RootCertificate) -> Self {
        let mut certificates = HashMap::new();
        certificates.insert(root.spki_hash().clone(), root.clone().to_certificate());
        Self { root, certificates }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transaction {
    Add(UnverifiedCertificate),
    RemoveExpired(UnverifiedCertificate),
}

impl secure_chain::Transaction for Transaction {
    fn digest(&self, digest: &mut impl sha2::Digest) {
        match self {
            Transaction::Add(certificate) => {
                digest.update(b"add");
                digest.update(certificate.as_der());
            }
            Transaction::RemoveExpired(certificate) => {
                digest.update(b"remove_expired");
                digest.update(certificate.as_der());
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("signer not known")]
    SignerNotKnown,
    #[error("issuer is not signer")]
    IssuerIsNotSigner,
    #[error("certificate already exists")]
    CertificateAlreadyExists,
    #[error("certificate invalid: {0}")]
    CertificateInvalid(#[from] SignatureVerificationError),
    #[error("certificate not expired")]
    CertificateNotExpired,
    #[error("certificate not found")]
    CertificateNotFound,
    #[error("certificate does not match")]
    CertificateDoesNotMatch,
}

impl ChainState for State {
    type Transaction = Transaction;
    type Error = Error;
    type Exported = ExportedState;
    type ImportError = ImportError;

    fn check(
        &self,
        signer: &SpkiHash,
        time: u64,
        transaction: &Self::Transaction,
    ) -> Result<(), Self::Error> {
        let signer = self.certificates.get(signer).ok_or(Error::SignerNotKnown)?;
        match transaction {
            Transaction::Add(certificate) => {
                if self.certificates.contains_key(certificate.spki_hash()) {
                    return Err(Error::CertificateAlreadyExists);
                }
                if signer.spki_hash() != certificate.issuer() {
                    return Err(Error::IssuerIsNotSigner);
                }
                certificate.clone().verify_signature(signer, time)?;
            }
            Transaction::RemoveExpired(certificate) => {
                let Some(found_certificate) = self.certificates.get(certificate.spki_hash()) else {
                    return Err(Error::CertificateNotFound);
                };
                if found_certificate != certificate {
                    return Err(Error::CertificateDoesNotMatch);
                }
                if certificate.check_validity_at(time).is_ok() {
                    return Err(Error::CertificateNotExpired);
                }
            }
        }

        Ok(())
    }

    fn apply(&mut self, _signer: &SpkiHash, time: u64, transaction: &Self::Transaction) {
        match transaction {
            Transaction::Add(unverified_certificate) => {
                let signer = self
                    .certificates
                    .get(unverified_certificate.issuer())
                    .expect("transaction already checked");
                let certificate = unverified_certificate
                    .clone()
                    .verify_signature(signer, time)
                    .expect("transaction already checked");
                self.certificates
                    .insert(certificate.spki_hash().clone(), certificate);
            }
            Transaction::RemoveExpired(certificate) => {
                self.certificates.remove(certificate.spki_hash());
            }
        }
    }

    fn revert(&mut self, transaction: &Self::Transaction) {
        match transaction {
            Transaction::Add(certificate) => {
                self.certificates.remove(certificate.spki_hash());
            }
            Transaction::RemoveExpired(certificate) => {
                self.certificates.insert(
                    certificate.spki_hash().clone(),
                    certificate.clone().mark_as_trusted(),
                );
            }
        }
    }

    fn digest(&self, digest: &mut impl sha2::Digest) {
        digest.update(self.root.as_der());
        let mut keys = self.certificates.keys().collect::<Vec<_>>();
        keys.sort();
        for key in keys {
            let certificate = self
                .certificates
                .get(&key)
                .expect("keys were already taken from map");
            digest.update(certificate.as_der());
        }
    }

    fn export(&self) -> Self::Exported {
        ExportedState {
            root: self.root.clone().to_unverified(),
            certificates: self
                .certificates
                .values()
                .map(|c| c.clone().to_unverified())
                .collect(),
        }
    }

    fn import(exported: Self::Exported) -> Result<Self, Self::ImportError> {
        let root = exported.root.use_as_root()?;
        let mut seen = HashMap::<&SpkiHash, bool>::new();

        for cert in &exported.certificates {
            let issuer = cert.issuer();
            seen.insert(cert.spki_hash(), true);
            seen.entry(&issuer).or_insert(false);
        }

        let missing: Vec<_> = seen
            .into_iter()
            .filter(|(_, seen)| *seen == false)
            .map(|(spki_hash, _)| spki_hash.clone())
            .collect();

        if !missing.is_empty() {
            return Err(ImportError::MissingIssuers(missing));
        }

        let certificates = exported
            .certificates
            .into_iter()
            .map(|cert| (cert.spki_hash().clone(), cert.mark_as_trusted()))
            .collect();

        Ok(Self { root, certificates })
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct ExportedState {
    root: UnverifiedCertificate,
    certificates: Vec<UnverifiedCertificate>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("root certificate error")]
    RootError(#[from] UseAsRootError),
    #[error("missing issuers: {0:?}")]
    MissingIssuers(Vec<SpkiHash>),
}

#[derive(Debug, thiserror::Error)]
pub enum CompleteCertChainError {
    #[error("issuer with spki hash {0} not found")]
    NotFound(SpkiHash),
    #[error("error adding cert to chain: {0}")]
    AddCertificateError(#[from] AddCertificateError),
}
