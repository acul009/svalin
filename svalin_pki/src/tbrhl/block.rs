use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::tbrhl::BLOCK_HASH_SIZE;

use super::{BlockHash, Transaction};

#[derive(Serialize, Deserialize)]
pub struct Block<T> {
    index: u64,
    timestamp: u128,
    transaction: T,
    previous_hash: BlockHash,
    hash: BlockHash,
}

impl<T> Block<T>
where
    T: Transaction,
{
    pub fn first(transaction: T) -> Block<T> {
        Block {
            index: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            transaction,
            previous_hash: [0; BLOCK_HASH_SIZE],
            hash: [0; BLOCK_HASH_SIZE],
        }
    }

    pub fn successor(&self, transaction: T) -> Result<Block<T>> {
        let index = self.index + 1;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let previous_hash = self.hash;

        let hash = Self::create_hash(&index, &timestamp, &transaction, &previous_hash)?;

        Ok(Self {
            index,
            timestamp,
            transaction,
            previous_hash,
            hash,
        })
    }

    fn create_hash(
        index: &u64,
        timestamp: &u128,
        transaction: &T,
        previous_hash: &BlockHash,
    ) -> Result<BlockHash> {
        let mut to_hash = Vec::<u8>::new();
        to_hash.extend_from_slice(index.to_be_bytes().as_ref());
        to_hash.extend_from_slice(timestamp.to_be_bytes().as_ref());
        to_hash.extend_from_slice(transaction.hash()?.as_ref());
        to_hash.extend_from_slice(previous_hash.as_ref());

        let hash = ring::digest::digest(&ring::digest::SHA256, &to_hash);
        let hash: BlockHash = hash.as_ref()[0..BLOCK_HASH_SIZE].try_into()?;

        Ok(hash)
    }

    pub fn transaction(&self) -> &T {
        &self.transaction
    }

    pub fn hash(&self) -> &BlockHash {
        &self.hash
    }

    pub fn verify(&self, previous_hash: &BlockHash) -> Result<()> {
        if previous_hash != &self.previous_hash {
            return Err(anyhow::anyhow!("previous hash mismatch"));
        }

        let hash = Self::create_hash(
            &self.index,
            &self.timestamp,
            &self.transaction,
            &self.previous_hash,
        )?;

        if hash != self.hash {
            return Err(anyhow::anyhow!("hash mismatch"));
        }

        Ok(())
    }
}
