use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};

use super::{BlockHash, BLOCK_HASH_SIZE};

pub trait Transaction: Serialize + DeserializeOwned {
    fn hash(&self) -> Result<BlockHash> {
        let serialized = postcard::to_extend(self, Vec::new())?;

        let hash = ring::digest::digest(&ring::digest::SHA512_256, &serialized);
        let hash: BlockHash = hash.as_ref()[0..BLOCK_HASH_SIZE].try_into()?;

        Ok(hash)
    }
}
