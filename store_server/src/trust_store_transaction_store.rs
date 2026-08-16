use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use svalin_pki::{secure_chain::CheckedBlock, trust_store};

#[derive(Debug)]
pub struct TrustStoreTransactionStore {
    pool: sqlx::SqlitePool,
    current_sequence: AtomicU64,
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
    pub async fn open(pool: sqlx::SqlitePool) -> Result<Arc<Self>, sqlx::Error> {
        let current_sequence = sqlx::query_scalar!(
            r#"
            SELECT MAX(sequence) FROM trust_store_transactions
            "#
        )
        .fetch_one(&pool)
        .await?
        .unwrap_or(0);

        Ok(Arc::new(Self {
            pool,
            current_sequence: AtomicU64::new(current_sequence as u64),
        }))
    }

    pub async fn add(
        &self,
        transaction: &CheckedBlock<trust_store::Transaction>,
    ) -> Result<(), TransactionStoreError> {
        if transaction.sequence() != self.current_sequence.load(Ordering::Relaxed) + 1 {
            return Err(TransactionStoreError::SequenceMismatch);
        }
        let data = postcard::to_stdvec(&transaction.as_unchecked())?;
        sqlx::query!(
            "INSERT INTO trust_store_transactions (sequence, data) VALUES (?, ?)",
            transaction.sequence() as i64,
            &data
        )
        .execute(&self.pool)
        .await?;
        self.current_sequence
            .store(transaction.sequence(), Ordering::Relaxed);
        Ok(())
    }
}
