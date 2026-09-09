use sqlx::SqlitePool;
use std::{fmt::Debug, path::Path, sync::Arc};

use super::{close_handle::CloseHandle, trust_store_transaction_store::TrustStoreTransactionStore};

pub struct AgentStore {
    pool: SqlitePool,
    transaction_store: Arc<TrustStoreTransactionStore>,
}

impl AgentStore {
    pub async fn open(filename: impl AsRef<Path>) -> Result<Self, Error> {
        let pool = super::open_database(filename).await?;

        Ok(Self {
            transaction_store: Arc::new(TrustStoreTransactionStore::open(pool.clone()).await?),
            pool,
        })
    }

    pub fn transaction_store(&self) -> &Arc<TrustStoreTransactionStore> {
        &self.transaction_store
    }

    pub fn close_handle(&self) -> CloseHandle {
        CloseHandle(self.pool.clone())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("MessagePack encode error: {0}")]
    MessagePackEncode(#[from] rmp_serde::encode::Error),
    #[error("MessagePack decode error: {0}")]
    MessagePackDecode(#[from] rmp_serde::decode::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}
