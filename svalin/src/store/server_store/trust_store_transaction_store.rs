use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use svalin_pki::{
    secure_chain::{CheckedBlock, UncheckedBlock},
    trust_store,
};
use tokio::sync::broadcast;

#[derive(Debug)]
pub struct TrustStoreTransactionStore {
    pool: sqlx::SqlitePool,
    current_sequence: AtomicU64,
    broadcast: broadcast::Sender<Arc<UncheckedBlock<trust_store::Transaction>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionStoreError {
    #[error("SQLx error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("Sequence mismatch: expected {expected}, got {actual}")]
    SequenceMismatch { expected: u64, actual: u64 },
    #[error("Postcard error: {0}")]
    PostcardError(#[from] postcard::Error),
    #[error("Broadcast error")]
    BroadcastError,
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
        .unwrap_or(1);

        Ok(Arc::new(Self {
            pool,
            current_sequence: AtomicU64::new(current_sequence as u64),
            broadcast: broadcast::channel(100).0,
        }))
    }

    pub async fn add_and_broadcast(
        &self,
        transaction: CheckedBlock<trust_store::Transaction>,
    ) -> Result<(), TransactionStoreError> {
        if transaction.sequence() != self.current_sequence.load(Ordering::Relaxed) + 1 {
            return Err(TransactionStoreError::SequenceMismatch {
                expected: self.current_sequence.load(Ordering::Relaxed) + 1,
                actual: transaction.sequence(),
            });
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
        self.broadcast
            .send(Arc::new(transaction.to_unchecked()))
            .map_err(|_| TransactionStoreError::BroadcastError)?;
        Ok(())
    }

    pub async fn load_all_after(
        &self,
        after: u64,
    ) -> Result<
        (
            Vec<UncheckedBlock<trust_store::Transaction>>,
            broadcast::Receiver<Arc<UncheckedBlock<trust_store::Transaction>>>,
        ),
        LoadTransactionError,
    > {
        let mut receiver = self.broadcast.subscribe();
        let transactions = sqlx::query!(
            "SELECT data FROM trust_store_transactions WHERE sequence > ? ORDER BY sequence ASC",
            after as i64
        )
        .fetch_all(&self.pool)
        .await?;

        let mut transactions = transactions
            .into_iter()
            .map(|record| {
                let block: UncheckedBlock<trust_store::Transaction> =
                    postcard::from_bytes(&record.data)?;
                Ok(block)
            })
            .collect::<Result<Vec<_>, postcard::Error>>()?;

        while let Ok(block) = receiver.try_recv() {
            if block.sequence() > transactions.last().map(|b| b.sequence()).unwrap_or(after) {
                transactions.push(block.deref().clone());
                break;
            }
        }

        Ok((transactions, receiver))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadTransactionError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("postcard error: {0}")]
    Postcard(#[from] postcard::Error),
}
