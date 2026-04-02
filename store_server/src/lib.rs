mod agent_store;
mod key_package_store;
mod message_store;
mod session_store;
mod user_store;

pub use agent_store::{AddAgentError, AgentStore, AgentUpdate};
pub use key_package_store::KeyPackageStore;
pub use message_store::{MessageStore, MessageStoreError};
pub use session_store::{AddSessionError, SessionStore};
pub use user_store::{CompleteCertChainError, GetBySpkiHashError, UserStore};

use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::{fmt::Debug, path::Path, sync::Arc};

pub struct ServerStore {
    pub agents: Arc<AgentStore>,
    pub key_packages: Arc<KeyPackageStore>,
    pub messages: Arc<MessageStore>,
    pub sessions: Arc<SessionStore>,
    pub users: Arc<UserStore>,
    pool: SqlitePool,
}

impl ServerStore {
    pub async fn open(filename: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::new()
            .create_if_missing(true)
            .filename(filename)
            .optimize_on_close(true, None);

        let pool = SqlitePool::connect_with(options).await?;
        sqlx::migrate!().run(&pool).await?;

        Ok(Self {
            agents: AgentStore::open(pool.clone()),
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

pub struct CloseHandle(SqlitePool);

impl Debug for CloseHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloseHandle").finish()
    }
}

impl CloseHandle {
    pub async fn close(&self) {
        self.0.close().await
    }
}
