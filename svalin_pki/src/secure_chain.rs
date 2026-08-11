use std::marker::PhantomData;

use rand::rand_core::block;
use ring::signature::{ED25519, VerificationAlgorithm};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

use crate::{Certificate, Credential, SpkiHash, get_current_timestamp};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainDigest(pub [u8; 64]);
#[derive(Clone, Debug, PartialEq, Eq)]
struct StateDigest(pub [u8; 64]);
#[derive(Clone, Debug, PartialEq, Eq)]
struct BlockDigest(pub [u8; 64]);
impl BlockDigest {
    fn empty() -> Self {
        Self([0; 64])
    }
}

pub trait ChainState: Serialize + DeserializeOwned {
    type Transaction: Transaction + Serialize + DeserializeOwned + Clone;
    type Error: std::error::Error;

    fn check(
        &self,
        signer: &SpkiHash,
        time: u64,
        transaction: &Self::Transaction,
    ) -> Result<(), Self::Error>;
    fn apply(&mut self, signer: &SpkiHash, time: u64, transaction: &Self::Transaction);
    fn revert(&mut self, transaction: &Self::Transaction);
    fn digest(&self, digest: &mut impl Digest);
}

pub trait Transaction {
    fn digest(&self, digest: &mut impl Digest);
}

pub struct Chain<State: ChainState> {
    last_block: Option<CheckedBlock<State::Transaction>>,
    state: State,
}

impl<State: ChainState> Chain<State> {
    pub fn initialize(state: State) -> Self {
        Self {
            last_block: None,
            state,
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn package(
        &mut self,
        transaction: State::Transaction,
        credential: &Credential,
    ) -> Result<CheckedBlock<State::Transaction>, CreateBlockError<State::Error>> {
        let timestamp = get_current_timestamp();
        if let Err(reason) = self.state.check(
            credential.certificate().spki_hash(),
            timestamp,
            &transaction,
        ) {
            return Err(CreateBlockError::InvalidTransaction(reason));
        }

        self.state.apply(
            credential.certificate().spki_hash(),
            timestamp,
            &transaction,
        );
        let mut hasher = Sha512::new();
        self.state.digest(&mut hasher);
        let new_digest = StateDigest(hasher.finalize().into());
        self.state.revert(&transaction);

        let mut block = if let Some(last) = &self.last_block {
            UncheckedBlock {
                time: timestamp,
                sequence: last.0.sequence + 1,
                previous_block_hash: last.0.digest(),
                resulting_state: new_digest,
                signer: credential.certificate().spki_hash().clone(),
                transaction,
                signature: Vec::new(),
            }
        } else {
            UncheckedBlock {
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

        Ok(CheckedBlock(block))
    }

    pub fn check(
        &mut self,
        block: UncheckedBlock<State::Transaction>,
        certificate: &Certificate,
    ) -> Result<CheckedBlock<State::Transaction>, CheckBlockError<State::Error>> {
        if let Some(last) = &self.last_block {
            if block.sequence != last.0.sequence + 1 {
                return Err(CheckBlockError::SequenceMismatch);
            }
            if block.previous_block_hash != last.0.digest() {
                return Err(CheckBlockError::PreviousBlockHashMismatch);
            }
            // Todo: figure out if equal times are allowed here. Currently they are for testing.
            if block.time < last.0.time {
                return Err(CheckBlockError::TimeToEarly(block.time, last.0.time));
            }
        } else {
            if block.sequence != 0 {
                return Err(CheckBlockError::SequenceMismatch);
            }
            if block.previous_block_hash != BlockDigest::empty() {
                return Err(CheckBlockError::PreviousBlockHashMismatch);
            }
        }

        if block.signer() != certificate.spki_hash() {
            return Err(CheckBlockError::IncorrectCertificate);
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
            return Err(CheckBlockError::InvalidTransaction(reason));
        }

        self.state
            .apply(&block.signer, block.time, &block.transaction);

        let mut hasher = Sha512::new();
        self.state.digest(&mut hasher);
        let new_digest = StateDigest(hasher.finalize().into());
        self.state.revert(&block.transaction);

        if block.resulting_state != new_digest {
            return Err(CheckBlockError::ResultingStateMismatch);
        }

        Ok(CheckedBlock(block))
    }

    pub fn apply(&mut self, block: CheckedBlock<State::Transaction>) {
        self.state
            .apply(&block.0.signer, block.0.time, &block.0.transaction);

        let mut hasher = Sha512::new();
        self.state.digest(&mut hasher);
        let new_digest = StateDigest(hasher.finalize().into());

        if block.0.resulting_state != new_digest {
            panic!("resulting state mismatch should have already been checked")
        }

        self.last_block = Some(block);
    }

    pub(crate) fn digest(&self) -> ChainDigest {
        let block_digest = self
            .last_block
            .as_ref()
            .map(|block| block.0.digest())
            .unwrap_or_else(|| BlockDigest::empty());
        let mut digest = Sha512::new().chain_update(block_digest.0);
        self.state.digest(&mut digest);
        ChainDigest(digest.finalize().into())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CheckBlockError<Inner> {
    #[error("block sequence mismatch")]
    SequenceMismatch,
    #[error("previous block hash mismatch")]
    PreviousBlockHashMismatch,
    #[error("block time {0} is earlier than previous block time {1}")]
    TimeToEarly(u64, u64),
    #[error("incorrect certificate given")]
    IncorrectCertificate,
    #[error("signature verification failed")]
    SignatureVerificationFailed(ring::error::Unspecified),
    #[error("invalid transaction: {0}")]
    InvalidTransaction(Inner),
    #[error("resulting state mismatch")]
    ResultingStateMismatch,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateBlockError<Inner> {
    #[error("invalid transaction: {0}")]
    InvalidTransaction(Inner),
}

impl<Inner> From<ring::error::Unspecified> for CheckBlockError<Inner> {
    fn from(err: ring::error::Unspecified) -> Self {
        Self::SignatureVerificationFailed(err)
    }
}

#[derive(Debug)]
pub struct CheckedBlock<T>(UncheckedBlock<T>);

impl<T> CheckedBlock<T> {
    pub fn as_unchecked(&self) -> &UncheckedBlock<T> {
        &self.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UncheckedBlock<T> {
    sequence: u64,
    time: u64,
    previous_block_hash: BlockDigest,
    resulting_state: StateDigest,
    signer: SpkiHash,
    transaction: T,
    /// Not part of the BlockDigest
    signature: Vec<u8>,
}

impl<T: Transaction> UncheckedBlock<T> {
    pub fn signer(&self) -> &SpkiHash {
        &self.signer
    }

    pub fn digest(&self) -> BlockDigest {
        let mut hasher = Sha512::new()
            .chain_update(self.sequence.to_le_bytes())
            .chain_update(self.time.to_le_bytes())
            .chain_update(self.previous_block_hash.0)
            .chain_update(self.resulting_state.0)
            .chain_update(self.signer.as_slice());
        self.transaction.digest(&mut hasher);
        BlockDigest(hasher.finalize().into())
    }
}

use serde::de::{DeserializeOwned, Error, Visitor};
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

impl Serialize for ChainDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for ChainDigest {
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

impl<State: ChainState> Serialize for Chain<State> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = serializer.serialize_struct("Chain", 2)?;
        ser.serialize_field(
            "last_block",
            &self.last_block.as_ref().map(|block| block.as_unchecked()),
        )?;
        ser.serialize_field("state", &self.state)?;
        ser.end()
    }
}

impl<'de, State: ChainState> Deserialize<'de> for Chain<State> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "Chain",
            &["last_block", "state"],
            ChainVisitor::<State> {
                _marker: PhantomData,
            },
        )
    }
}

struct ChainVisitor<State: ChainState> {
    _marker: PhantomData<State>,
}

impl<'de, State: ChainState> Visitor<'de> for ChainVisitor<State> {
    type Value = Chain<State>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a chain")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let Some(last_block) = seq.next_element::<Option<UncheckedBlock<State::Transaction>>>()?
        else {
            return Err(A::Error::custom("expected last_block option"));
        };
        let Some(state) = seq.next_element::<State>()? else {
            return Err(A::Error::custom("expected state"));
        };

        if let Some(last_block) = &last_block {
            let mut digest = Sha512::new();
            state.digest(&mut digest);
            let digest: [u8; 64] = digest.finalize().into();
            if last_block.resulting_state.0 != digest {
                return Err(A::Error::custom("state digest does not match last_block"));
            }
        }

        Ok(Chain {
            last_block: last_block.map(CheckedBlock),
            state,
        })
    }
}
