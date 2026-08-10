use std::{collections::HashMap, fmt};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, SeqAccess, Visitor},
    ser::SerializeStruct,
};

use crate::{
    Certificate, RootCertificate, SignatureVerificationError, SpkiHash, UnverifiedCertificate,
    certificate,
    secure_chain::{self, ApplyBlockError, Block, Chain, ChainState, CreateBlockError},
};

pub struct TrustStore {
    chain: Chain<State>,
}

impl TrustStore {
    pub fn initialize(root: RootCertificate) -> Self {
        let mut certificates = HashMap::new();
        certificates.insert(root.spki_hash().clone(), root.as_certificate().clone());
        let state = State { root, certificates };

        Self {
            chain: Chain::initialize(state),
        }
    }

    pub fn try_apply(&mut self, update: Block<Transaction>) -> Result<(), ApplyBlockError<Error>> {
        let signer = self
            .chain
            .state()
            .certificates
            .get(update.signer())
            .ok_or(ApplyBlockError::InvalidTransaction(Error::SignerNotKnown))?
            .clone();
        self.chain.try_apply(update, &signer)?;

        Ok(())
    }

    pub fn add(
        &mut self,
        certificate: Certificate,
    ) -> Result<Block<Transaction>, CreateBlockError<Error>> {
        self.chain
            .apply_and_package(Transaction::Add(certificate.to_unverified()))
    }
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
        for certificate in self.certificates.values() {
            digest.update(certificate.as_der());
        }
    }
}

impl Serialize for State {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("trust::state", 2)?;
        state.serialize_field("root", &self.root.as_unverified())?;
        state.serialize_field(
            "certificates",
            &self
                .certificates
                .iter()
                .map(|cert| cert.1.clone().to_unverified())
                .collect::<Vec<_>>(),
        )?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for State {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let state: Self = deserializer.deserialize_struct(
            "trust::state",
            &["root", "certificates"],
            StateVisitor,
        )?;

        Ok(state)
    }
}

struct StateVisitor;

impl<'de> Visitor<'de> for StateVisitor {
    type Value = State;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct trust::state")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<State, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let root: UnverifiedCertificate = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;

        let certificates: Vec<UnverifiedCertificate> = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;

        let mut state = State {
            root: root
                .use_as_root()
                .expect("root certificate is not a root certificate"),
            certificates: HashMap::new(),
        };

        for certificate in certificates {
            state.certificates.insert(
                certificate.spki_hash().clone(),
                certificate.mark_as_trusted(),
            );
        }

        Ok(state)
    }
}

#[cfg(test)]
mod test {
    use sha2::{Digest, Sha512};

    use crate::{Credential, KeyPair, get_current_timestamp};

    use super::*;

    #[test]
    fn test_serde() {
        let root = Credential::generate_root().unwrap();
        let keypair = KeyPair::generate();
        let cert = root
            .create_agent_certificate_for_key(&keypair.export_public_key())
            .unwrap();
        let mut state = State::initialize(
            root.certificate()
                .clone()
                .to_unverified()
                .use_as_root()
                .unwrap(),
        );
        state.apply(
            root.certificate().spki_hash(),
            get_current_timestamp(),
            &Transaction::Add(cert.to_unverified()),
        );

        let serialized = postcard::to_stdvec(&state).unwrap();
        let deserialized: State = postcard::from_bytes(&serialized).unwrap();
        let mut hasher1 = Sha512::new();
        state.digest(&mut hasher1);
        let hash1 = hasher1.finalize();
        let mut hasher2 = Sha512::new();
        deserialized.digest(&mut hasher2);
        let hash2 = hasher2.finalize();

        assert_eq!(hash1, hash2);
    }
}
