use std::path::Path;

use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};

pub mod agent_store;
pub mod client_store;
mod close_handle;
pub mod server_store;
pub mod trust_store_transaction_store;

pub use close_handle::CloseHandle;

async fn open_database(filename: impl AsRef<Path>) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .create_if_missing(true)
        .filename(filename)
        .optimize_on_close(true, None);

    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(sqlx::Error::from)?;

    Ok(pool)
}
