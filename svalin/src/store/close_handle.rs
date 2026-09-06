use std::fmt::Debug;

use sqlx::SqlitePool;

pub struct CloseHandle(pub(crate) SqlitePool);

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
