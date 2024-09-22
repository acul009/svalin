use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{signed_object::SignedObject, tbrhl::BLOCK_HASH_SIZE, Certificate, PermCredentials};

use super::{BlockHash, Transaction};

#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Block<T>
where
    T: Transaction,
{
    data: SignedObject<BlockData<T>>,
}

#[derive(Serialize, Deserialize)]
pub struct BlockData<T> {
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
    fn first(transaction: T, credentials: &PermCredentials) -> Result<Block<T>> {
        let index = 0;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let previous_hash = [0; BLOCK_HASH_SIZE];
        let hash = Self::create_hash(index, timestamp, &transaction, &previous_hash)?;

        let data = BlockData {
            index,
            timestamp,
            transaction,
            previous_hash,
            hash,
        };

        let signed = SignedObject::new(data, credentials)?;

        Ok(Block { data: signed })
    }

    pub fn successor(&self, transaction: T, credentials: &PermCredentials) -> Result<Block<T>> {
        let index = self.data.index + 1;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let previous_hash = self.data.hash;

        let hash = Self::create_hash(index, timestamp, &transaction, &previous_hash)?;

        let data = BlockData {
            index,
            timestamp,
            previous_hash,
            hash,
            transaction,
        };

        let signed = SignedObject::new(data, credentials)?;

        Ok(Block { data: signed })
    }

    fn create_hash(
        index: u64,
        timestamp: u128,
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
        &self.data.transaction
    }

    pub fn signer(&self) -> &Certificate {
        self.data.signed_by()
    }

    pub fn verify_as_successor(&self, previous: &Block<T>) -> Result<()> {
        if self.data.index != previous.data.index + 1 {
            return Err(anyhow::anyhow!("index mismatch"));
        }

        if self.data.timestamp <= previous.data.timestamp {
            return Err(anyhow!("timestamp mismatch"));
        }

        if self.data.previous_hash != previous.data.hash {
            return Err(anyhow::anyhow!("previous hash mismatch"));
        }

        let hash = Self::create_hash(
            self.data.index,
            self.data.timestamp,
            &self.data.transaction,
            &self.data.previous_hash,
        )?;

        if hash != self.data.hash {
            return Err(anyhow::anyhow!("hash mismatch"));
        }

        Ok(())
    }

    /// TODO: verify signature and signer
    pub fn verify_as_first(&self) -> Result<()> {
        if self.data.index != 0 {
            return Err(anyhow!("index of first block not 0"));
        }

        if self.data.previous_hash != [0; BLOCK_HASH_SIZE] {
            return Err(anyhow!("previous hash of first block not 0"));
        }

        let hash = Self::create_hash(
            self.data.index,
            self.data.timestamp,
            &self.data.transaction,
            &self.data.previous_hash,
        )?;

        if hash != self.data.hash {
            return Err(anyhow::anyhow!("hash mismatch"));
        }

        Ok(())
    }
}
