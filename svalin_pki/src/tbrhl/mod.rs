use anyhow::{anyhow, Context, Result};
use block::Block;
use blockstore::BlockStore;
use handler::TransactionHandler;
use transaction::Transaction;

use crate::PermCredentials;

mod block;
mod blockstore;
pub mod handler;
pub mod transaction;

pub const BLOCK_HASH_SIZE: usize = 32;
pub type BlockHash = [u8; BLOCK_HASH_SIZE];

/// **Transaction Based Rolling Hash Ledger**
///
/// This system is derived from the Blockchain system.
/// It maintains a state that is updated by applying transactions.
/// Unwanted changes can be detected by verifying the
pub struct TBRHL<Store, Handler>
where
    Handler: TransactionHandler,
{
    store: Store,
    handler: Handler,
    state: Handler::State,
}

impl<T, Store, Handler> TBRHL<Store, Handler>
where
    T: Transaction,
    Store: BlockStore<T = T>,
    Handler: TransactionHandler<T = T>,
{
    pub async fn open(store: Store, handler: Handler) -> Result<Self> {
        let first_block = store
            .get(0)
            .await
            .ok_or_else(|| anyhow!("missing first block"))?;

        first_block
            .verify_as_first()
            .context("failed to verify first block")?;

        let mut state = handler.init(first_block.transaction(), first_block.signer())?;

        let mut previous_block = first_block;

        for i in 0..store.len() {
            let block = store
                .get(i)
                .await
                .ok_or_else(|| anyhow::anyhow!("block {i} not found"))?;

            if let Err(err) = block.verify_as_successor(&previous_block) {
                return Err(err.context(format!("failed to verify block {i}")));
            }

            if let Err(err) = handler.verify(&state, block.transaction(), block.signer()) {
                return Err(err.context(format!("failed to verify transaction of block {i}")));
            }

            handler.apply(&mut state, block.transaction())?;

            previous_block = block;
        }

        Ok(Self {
            store,
            handler,
            state,
        })
    }

    pub fn len(&self) -> u64 {
        self.store.len()
    }

    pub async fn add_transaction(
        &mut self,
        transaction: T,
        signer: &PermCredentials,
    ) -> Result<()> {
        if let Err(err) = self
            .handler
            .verify(&self.state, &transaction, signer.get_certificate())
        {
            return Err(err.context("failed to verify transaction"));
        }

        let latest_block = self
            .store
            .get(self.store.len() - 1)
            .await
            .ok_or_else(|| anyhow!("no blocks found"))?;

        let new_block = latest_block.successor(transaction, signer)?;

        self.handler
            .apply(&mut self.state, new_block.transaction())?;

        self.store.add(new_block);

        Ok(())
    }

    pub fn state(&self) -> &Handler::State {
        &self.state
    }
}
