use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512_256};

use crate::signed_object::{SignedObject, SignedObjectError, VerifiedObject};
use crate::{Certificate, Credential, UnverifiedCertificate, get_current_timestamp};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainDigest {
    digest: [u8; 32],
    sequence: u64,
}
impl ChainDigest {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct BlockDigest([u8; 32]);
impl BlockDigest {
    fn empty() -> Self {
        Self([0; 32])
    }
}

pub trait ChainState: Sized {
    type Transaction: Transaction + Serialize + DeserializeOwned + Clone;
    type Error: std::error::Error;
    type Exported: Serialize + DeserializeOwned;
    type ImportError: std::error::Error;

    fn check(
        &self,
        signer: &Certificate,
        time: u64,
        transaction: &Self::Transaction,
    ) -> Result<(), Self::Error>;
    fn apply(&mut self, signer: &Certificate, time: u64, transaction: &Self::Transaction);
    fn revert(&mut self, transaction: &Self::Transaction);
    fn digest(&self, digest: &mut impl sha2::Digest);
    fn export(&self) -> Self::Exported;
    fn import(exported: Self::Exported) -> Result<Self, Self::ImportError>;
}

pub trait Transaction {
    fn digest(&self, digest: &mut impl sha2::Digest);
}

enum TimeRatchetMode {
    MinAge(u64),
    RealTime,
}

const MAX_REALTIME_OFFSET_SECONDS: u64 = 30;
const MAX_FUTURE_CLOCK_SKEW_SECONDS: u64 = 30;

impl TimeRatchetMode {
    fn is_allowed(&self, time: u64) -> bool {
        match self {
            TimeRatchetMode::MinAge(age) => time >= *age,
            TimeRatchetMode::RealTime => {
                time >= get_current_timestamp() - MAX_REALTIME_OFFSET_SECONDS
            }
        }
    }

    fn now() -> Self {
        TimeRatchetMode::MinAge(get_current_timestamp())
    }

    fn to_timestamp(&self) -> u64 {
        match self {
            TimeRatchetMode::MinAge(age) => *age,
            TimeRatchetMode::RealTime => get_current_timestamp() - MAX_REALTIME_OFFSET_SECONDS,
        }
    }
}

pub struct Chain<State: ChainState> {
    last_block: Option<CheckedBlock<State::Transaction>>,
    time_ratchet: TimeRatchetMode,
    state: State,
}

impl<State: ChainState> Chain<State> {
    pub fn initialize(state: State) -> Self {
        Self {
            last_block: None,
            time_ratchet: TimeRatchetMode::now(),
            state,
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn package(
        &self,
        transaction: State::Transaction,
        credential: &Credential,
    ) -> Result<CheckedBlock<State::Transaction>, CreateBlockError<State::Error>> {
        let timestamp = get_current_timestamp();
        if let Err(reason) = self
            .state
            .check(credential.certificate(), timestamp, &transaction)
        {
            return Err(CreateBlockError::InvalidTransaction(reason));
        }

        let block = if let Some(last) = &self.last_block {
            BlockData {
                time: timestamp,
                sequence: last.as_ref().sequence + 1,
                previous_block_hash: last.digest(),
                transaction,
            }
        } else {
            BlockData {
                time: timestamp,
                // sequence needs to start at 1, because 0 is reserved for the no block state
                sequence: 1,
                previous_block_hash: BlockDigest::empty(),
                transaction,
            }
        };

        let block = CheckedBlock::new(block, credential);

        Ok(block)
    }

    pub fn check(
        &self,
        block: UncheckedBlock<State::Transaction>,
        certificate: Certificate,
    ) -> Result<CheckedBlock<State::Transaction>, CheckBlockError<State::Error>> {
        let block = block.verify_signature(certificate)?;

        if let Some(last) = &self.last_block {
            if block.sequence != last.sequence + 1 {
                return Err(CheckBlockError::SequenceMismatch);
            }
            if block.previous_block_hash != last.digest() {
                return Err(CheckBlockError::PreviousBlockHashMismatch);
            }
            // Todo: figure out if equal times are allowed here. Currently they are for testing.
            if block.time < last.time {
                return Err(CheckBlockError::TimeToEarly(block.time, last.time));
            }
        } else {
            if block.sequence != 1 {
                return Err(CheckBlockError::SequenceMismatch);
            }
            if block.previous_block_hash != BlockDigest::empty() {
                return Err(CheckBlockError::PreviousBlockHashMismatch);
            }
        }

        // Allow for clock differences between the block signer and verifier.
        if block.time > get_current_timestamp().saturating_add(MAX_FUTURE_CLOCK_SKEW_SECONDS) {
            return Err(CheckBlockError::TimeInFuture);
        }
        if !self.time_ratchet.is_allowed(block.time) {
            return Err(CheckBlockError::TooOld);
        }

        if let Err(reason) = self
            .state
            .check(&block.signed_by(), block.time, &block.transaction)
        {
            return Err(CheckBlockError::InvalidTransaction(reason));
        }

        Ok(block)
    }

    pub fn apply(&mut self, block: CheckedBlock<State::Transaction>) {
        self.state
            .apply(&block.signed_by(), block.time, &block.transaction);

        self.last_block = Some(block);
    }

    pub(crate) fn digest(&self) -> ChainDigest {
        let mut ctx = Sha512_256::new();
        let block_digest = self
            .last_block
            .as_ref()
            .map_or(BlockDigest::empty(), |b| b.digest());
        ctx.update(block_digest.as_ref());
        self.state.digest(&mut ctx);
        let digest = ctx.finalize().into();

        ChainDigest {
            digest,
            sequence: self.sequence(),
        }
    }

    pub fn export(&self) -> ExportedChain<State> {
        let Some(last_block) = &self.last_block else {
            panic!("cannot export empty chain")
        };
        ExportedChain {
            state: self.state.export(),
            last_block_signer: last_block.signed_by().clone().to_unverified(),
            last_block: last_block.pack().clone(),
            time_ratchet: self.time_ratchet.to_timestamp(),
        }
    }

    pub fn import(exported: ExportedChain<State>) -> Result<Self, ImportError<State::ImportError>> {
        let state = State::import(exported.state).map_err(ImportError::ImportError)?;
        let signer = exported.last_block_signer.mark_as_trusted();
        let last_block = exported.last_block.verify_signature(signer)?;
        Ok(Self {
            state,
            time_ratchet: TimeRatchetMode::MinAge(exported.time_ratchet),
            last_block: Some(last_block),
        })
    }

    pub fn sequence(&self) -> u64 {
        self.last_block
            .as_ref()
            .map_or_else(|| 0, |block| block.sequence)
    }

    pub fn enable_real_time_ratchet(&mut self) {
        self.time_ratchet = TimeRatchetMode::RealTime;
    }
}

#[derive(Serialize, Deserialize)]
pub struct ExportedChain<State: ChainState> {
    state: State::Exported,
    last_block: UncheckedBlock<State::Transaction>,
    last_block_signer: UnverifiedCertificate,
    time_ratchet: u64,
}
impl<State> Clone for ExportedChain<State>
where
    State: ChainState,
    State::Exported: Clone,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            last_block: self.last_block.clone(),
            last_block_signer: self.last_block_signer.clone(),
            time_ratchet: self.time_ratchet,
        }
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
    SignatureVerificationFailed(#[from] SignedObjectError),
    #[error("invalid transaction: {0}")]
    InvalidTransaction(Inner),
    #[error("resulting state mismatch")]
    ResultingStateMismatch,
    #[error("block time is in the future")]
    TimeInFuture,
    #[error("block to old for time ratchet")]
    TooOld,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateBlockError<Inner> {
    #[error("invalid transaction: {0}")]
    InvalidTransaction(Inner),
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError<Inner> {
    #[error("import error: {0}")]
    ImportError(#[source] Inner),
    #[error("failed to verify imported block")]
    SignatureVerificationFailed(#[from] SignedObjectError),
    #[error("incorrect state digest")]
    IncorrectStateDigest,
}

pub type CheckedBlock<T> = VerifiedObject<BlockData<T>>;

impl<T: Serialize> CheckedBlock<T> {
    fn digest(&self) -> BlockDigest {
        self.pack().digest()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockData<T> {
    sequence: u64,
    time: u64,
    previous_block_hash: BlockDigest,
    transaction: T,
}

pub type UncheckedBlock<T> = SignedObject<BlockData<T>>;

impl<T: Serialize> UncheckedBlock<T> {
    fn digest(&self) -> BlockDigest {
        let serialized = rmp_serde::to_vec(&self).expect("serialization should not fail");
        let mut ctx = ring::digest::Context::new(&ring::digest::SHA512_256);
        ctx.update(&serialized);
        let digest = ctx.finish();

        BlockDigest(digest.as_ref().try_into().expect("Hash size is known"))
    }
}

impl AsRef<[u8]> for BlockDigest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
