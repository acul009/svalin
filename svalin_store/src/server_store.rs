mod key_package_store;
mod message_store;
mod session_store;
mod trust_store_transaction_store;
mod user_store;

pub use key_package_store::KeyPackageStore;
pub use message_store::{MessageStore, MessageStoreError};
pub use session_store::{AddSessionError, SessionStore};
pub use trust_store_transaction_store::{TransactionStoreError, TrustStoreTransactionStore};
pub use user_store::{GetBySpkiHashError, UserStore};

use sqlx::SqlitePool;
use std::{path::Path, sync::Arc};

use crate::close_handle::CloseHandle;

pub struct ServerStore {
    pub trust_store_transactions: Arc<TrustStoreTransactionStore>,
    pub key_packages: Arc<KeyPackageStore>,
    pub messages: Arc<MessageStore>,
    pub sessions: Arc<SessionStore>,
    pub users: Arc<UserStore>,
    pool: SqlitePool,
}

impl ServerStore {
    pub async fn open(filename: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let pool = super::open_database(filename).await?;

        Ok(Self {
            trust_store_transactions: TrustStoreTransactionStore::open(pool.clone()).await?,
            key_packages: KeyPackageStore::open(pool.clone()),
            messages: MessageStore::open(pool.clone()),
            sessions: SessionStore::open(pool.clone()),
            users: UserStore::open(pool.clone()),
            pool,
        })
    }

    pub fn close_handle(&self) -> CloseHandle {
        CloseHandle(self.pool.clone())
    }
}
