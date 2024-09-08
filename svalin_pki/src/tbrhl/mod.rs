use std::future::Future;

use anyhow::{anyhow, Result};
use block::Block;
use serde::{Deserialize, Serialize};

mod block;

pub const BLOCK_HASH_SIZE: usize = 32;
pub type BlockHash = [u8; BLOCK_HASH_SIZE];

pub trait Transaction: Serialize + for<'a> Deserialize<'a> {
    fn hash(&self) -> Result<BlockHash> {
        let serialized = postcard::to_extend(self, Vec::new())?;

        let hash = ring::digest::digest(&ring::digest::SHA256, &serialized);
        let hash: BlockHash = hash.as_ref()[0..BLOCK_HASH_SIZE].try_into()?;

        Ok(hash)
    }
}

pub trait BlockStore {
    type T: Transaction;

    fn len(&self) -> u64;

    fn get(&self, index: u64) -> impl Future<Output = Option<Block<Self::T>>>;
    fn add(&mut self, block: Block<Self::T>);
}

pub trait TransactionHandler {
    type T: Transaction;
    type State;

    fn verify(&self, state: &Self::State, transaction: &Self::T) -> Result<()>;

    fn apply(
        &self,
        state: &mut Self::State,
        transaction: &Self::T,
    ) -> impl Future<Output = Result<()>>;

    fn initial_state(&self) -> Self::State;
}

/// **Transaction Based Rolling Hash Ledger**
///
/// This system is derived from the Blockchain system.
/// It maintains a state that is updated by applying transactions.
/// Unwanted changes can be detected by verifying the
pub struct TBRHL<Store, Handler, State> {
    store: Store,
    handler: Handler,
    state: State,
    current_hash: BlockHash,
}

impl<T, Store, Handler, State> TBRHL<Store, Handler, State>
where
    T: Transaction,
    Store: BlockStore<T = T>,
    Handler: TransactionHandler<T = T, State = State>,
{
    pub async fn open(store: Store, handler: Handler) -> Result<Self> {
        let mut state = handler.initial_state();
        let mut previous_hash = [0u8; BLOCK_HASH_SIZE];
        for i in 0..store.len() {
            let block = store
                .get(i)
                .await
                .ok_or_else(|| anyhow::anyhow!("block {i} not found"))?;

            if let Err(err) = block.verify(&previous_hash) {
                return Err(err.context(format!("failed to verify block {i}")));
            }

            if let Err(err) = handler.verify(&state, block.transaction()) {
                return Err(err.context(format!("failed to verify transaction of block {i}")));
            }

            previous_hash = *block.hash();

            handler.apply(&mut state, block.transaction()).await?;
        }

        Ok(Self {
            store,
            handler,
            state,
            current_hash: previous_hash,
        })
    }

    pub fn len(&self) -> u64 {
        self.store.len()
    }

    pub async fn add_transaction(&mut self, transaction: T) -> Result<()> {
        if let Err(err) = self.handler.verify(&self.state, &transaction) {
            return Err(err.context("failed to verify transaction"));
        }

        let latest_block = self
            .store
            .get(self.store.len() - 1)
            .await
            .ok_or_else(|| anyhow!("no blocks found"))?;

        let new_block = latest_block.successor(transaction)?;

        self.handler
            .apply(&mut self.state, new_block.transaction())
            .await?;

        self.current_hash.clone_from(new_block.hash());

        self.store.add(new_block);

        Ok(())
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn current_hash(&self) -> BlockHash {
        self.current_hash
    }
}
