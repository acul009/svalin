use ring::signature::{ED25519, VerificationAlgorithm};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

use crate::{Certificate, Credential, SpkiHash, get_current_timestamp};

#[derive(Clone, PartialEq, Eq)]
pub struct StateDigest(pub [u8; 64]);
#[derive(Clone, PartialEq, Eq)]
pub struct BlockDigest(pub [u8; 64]);
impl BlockDigest {
    fn empty() -> Self {
        Self([0; 64])
    }
}

pub trait ChainState {
    type Transaction: Transaction + Serialize + DeserializeOwned + Clone;

    fn check(
        &self,
        signer: &SpkiHash,
        time: u64,
        transaction: &Self::Transaction,
    ) -> Result<(), String>;
    fn apply(&mut self, transaction: &Self::Transaction);
    fn revert(&mut self, transaction: &Self::Transaction);
    fn digest(&self, digest: &mut impl Digest);
}

pub trait Transaction {
    fn digest(&self, digest: &mut impl Digest);
}

#[derive(Serialize, Deserialize)]
pub struct Chain<State: ChainState> {
    last_block: Option<Block<State::Transaction>>,
    state: State,
}

impl<State: ChainState> Chain<State> {
    pub fn new(state: State) -> Self {
        Self {
            last_block: None,
            state,
        }
    }

    pub fn apply_and_package(
        &mut self,
        transaction: State::Transaction,
        credential: Credential,
    ) -> Result<Block<State::Transaction>, CreateBlockError> {
        let timestamp = get_current_timestamp();
        if let Err(reason) = self.state.check(
            credential.certificate().spki_hash(),
            timestamp,
            &transaction,
        ) {
            return Err(CreateBlockError::InvalidTransaction(reason));
        }

        self.state.apply(&transaction);
        let mut hasher = Sha512::new();
        self.state.digest(&mut hasher);
        let new_digest = StateDigest(hasher.finalize().into());

        let mut block = if let Some(last) = &self.last_block {
            Block {
                time: timestamp,
                sequence: last.sequence + 1,
                previous_block_hash: last.digest(),
                resulting_state: new_digest,
                signer: credential.certificate().spki_hash().clone(),
                transaction,
                signature: Vec::new(),
            }
        } else {
            Block {
                time: timestamp,
                sequence: 0,
                previous_block_hash: BlockDigest::empty(),
                resulting_state: new_digest,
                signer: credential.certificate().spki_hash().clone(),
                transaction,
                signature: Vec::new(),
            }
        };

        let digest = block.digest();
        let signature = credential
            .keypair()
            .signing_keypair()
            .sign(&digest.0)
            .as_ref()
            .to_vec();
        block.signature = signature;

        self.last_block = Some(block.clone());

        Ok(block)
    }

    pub fn try_apply(
        &mut self,
        block: Block<State::Transaction>,
        certificate: &Certificate,
    ) -> Result<(), ApplyBlockError> {
        if let Some(last) = &self.last_block {
            if block.sequence != last.sequence + 1 {
                return Err(ApplyBlockError::SequenceMismatch);
            }
            if block.previous_block_hash != last.digest() {
                return Err(ApplyBlockError::PreviousBlockHashMismatch);
            }
        } else {
            if block.sequence != 0 {
                return Err(ApplyBlockError::SequenceMismatch);
            }
            if block.previous_block_hash != BlockDigest::empty() {
                return Err(ApplyBlockError::PreviousBlockHashMismatch);
            }
        }

        if block.signer() != certificate.spki_hash() {
            return Err(ApplyBlockError::IncorrectCertificate);
        }

        let digest = block.digest();
        ED25519.verify(
            certificate.public_key().into(),
            digest.0.as_slice().into(),
            block.signature.as_slice().into(),
        )?;

        if let Err(reason) = self
            .state
            .check(&block.signer, block.time, &block.transaction)
        {
            return Err(ApplyBlockError::InvalidTransaction(reason));
        }

        self.state.apply(&block.transaction);

        let mut hasher = Sha512::new();
        self.state.digest(&mut hasher);
        let new_digest = StateDigest(hasher.finalize().into());

        if block.resulting_state != new_digest {
            self.state.revert(&block.transaction);
            return Err(ApplyBlockError::ResultingStateMismatch);
        }

        self.last_block = Some(block);

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyBlockError {
    #[error("block sequence mismatch")]
    SequenceMismatch,
    #[error("previous block hash mismatch")]
    PreviousBlockHashMismatch,
    #[error("incorrect certificate given")]
    IncorrectCertificate,
    #[error("signature verification failed")]
    SignatureVerificationFailed(ring::error::Unspecified),
    #[error("invalid transaction: {0}")]
    InvalidTransaction(String),
    #[error("resulting state mismatch")]
    ResultingStateMismatch,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateBlockError {
    #[error("invalid transaction: {0}")]
    InvalidTransaction(String),
}

impl From<ring::error::Unspecified> for ApplyBlockError {
    fn from(err: ring::error::Unspecified) -> Self {
        Self::SignatureVerificationFailed(err)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Block<T> {
    sequence: u64,
    time: u64,
    previous_block_hash: BlockDigest,
    resulting_state: StateDigest,
    signer: SpkiHash,
    transaction: T,
    /// Not part of the BlockDigest
    signature: Vec<u8>,
}

impl<T: Transaction> Block<T> {
    pub fn signer(&self) -> &SpkiHash {
        &self.signer
    }

    pub fn digest(&self) -> BlockDigest {
        let mut hasher = Sha512::new();
        hasher.update(self.sequence.to_le_bytes());
        hasher.update(self.time.to_le_bytes());
        hasher.update(self.previous_block_hash.0);
        hasher.update(self.resulting_state.0);
        hasher.update(self.signer.as_slice());
        self.transaction.digest(&mut hasher);
        BlockDigest(hasher.finalize().into())
    }
}

use serde::de::{DeserializeOwned, Error};
use serde::{Deserializer, Serializer};

impl Serialize for StateDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for StateDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: &[u8] = Deserialize::deserialize(deserializer)?;

        let bytes: [u8; 64] = bytes
            .try_into()
            .map_err(|_| D::Error::custom("expected 64 bytes"))?;

        Ok(Self(bytes))
    }
}

impl Serialize for BlockDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for BlockDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: &[u8] = Deserialize::deserialize(deserializer)?;

        let bytes: [u8; 64] = bytes
            .try_into()
            .map_err(|_| D::Error::custom("expected 64 bytes"))?;

        Ok(Self(bytes))
    }
}
