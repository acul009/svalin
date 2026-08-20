use std::sync::atomic::{AtomicU64, Ordering};

use svalin_pki::{
    secure_chain::{CheckedBlock, UncheckedBlock},
    trust_store,
};
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct TrustStoreTransactionStore {
    pool: sqlx::SqlitePool,
    current_sequence: tokio::sync::Mutex<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionStoreError {
    #[error("SQLx error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("Sequence mismatch")]
    SequenceMismatch,
    #[error("Postcard error: {0}")]
    PostcardError(#[from] postcard::Error),
}

impl TrustStoreTransactionStore {
    pub async fn open(pool: sqlx::SqlitePool) -> Result<Self, sqlx::Error> {
        let current_sequence = sqlx::query_scalar!(
            r#"
            SELECT MAX(sequence) FROM trust_store_transactions
            "#
        )
        .fetch_one(&pool)
        .await?
        .unwrap_or(1);

        Ok(Self {
            pool,
            current_sequence: Mutex::new(current_sequence as u64),
        })
    }

    pub async fn add(
        &self,
        transaction: &CheckedBlock<trust_store::Transaction>,
    ) -> Result<(), TransactionStoreError> {
        let mut current_sequence = self.current_sequence.lock().await;
        if transaction.sequence() > *current_sequence + 1 {
            return Err(TransactionStoreError::SequenceMismatch);
        }
        if transaction.sequence() <= *current_sequence {
            let data: Vec<u8> = sqlx::query_scalar!(
                "SELECT data FROM trust_store_transactions WHERE sequence = ?",
                transaction.sequence() as i64
            )
            .fetch_one(&self.pool)
            .await?;
            let block: UncheckedBlock<trust_store::Transaction> = postcard::from_bytes(&data)?;
            if &block == transaction.as_unchecked() {
                return Ok(());
            }
        }
        let data = postcard::to_stdvec(&transaction.as_unchecked())?;
        sqlx::query!(
            "INSERT INTO trust_store_transactions (sequence, data) VALUES (?, ?)",
            transaction.sequence() as i64,
            &data
        )
        .execute(&self.pool)
        .await?;
        *current_sequence = transaction.sequence();
        Ok(())
    }

    pub async fn load_all_after(
        &self,
        after: u64,
    ) -> Result<Vec<UncheckedBlock<trust_store::Transaction>>, LoadTransactionError> {
        let transactions = sqlx::query!(
            "SELECT data FROM trust_store_transactions WHERE sequence > ? ORDER BY sequence ASC",
            after as i64
        )
        .fetch_all(&self.pool)
        .await?;

        let transactions = transactions
            .into_iter()
            .map(|record| {
                let block: UncheckedBlock<trust_store::Transaction> =
                    postcard::from_bytes(&record.data)?;
                Ok(block)
            })
            .collect::<Result<Vec<_>, postcard::Error>>()?;

        Ok(transactions)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadTransactionError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("postcard error: {0}")]
    Postcard(#[from] postcard::Error),
}
