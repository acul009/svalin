use std::sync::Arc;

use svalin_pki::{get_current_timestamp, mls::transport_types::MessageToSend};
use uuid::Uuid;

#[derive(Debug)]
pub struct MessageStore {
    pool: sqlx::SqlitePool,
}

impl MessageStore {
    pub fn open(pool: sqlx::SqlitePool) -> Arc<Self> {
        Arc::new(Self { pool })
    }

    pub async fn add_message(&self, message: MessageToSend) -> Result<(), MessageStoreError> {
        let mut tx = self.pool.begin().await?;
        let message_id = Uuid::new_v4();
        let received_at = get_current_timestamp() as i64;
        let data = postcard::to_stdvec(&message.message)?;
        let data = &data;

        sqlx::query!(
            "INSERT INTO mls_messages (id, data, received_at) VALUES (?,?,?)",
            message_id,
            data,
            received_at
        )
        .execute(&mut *tx)
        .await?;

        for receiver in message.receivers {
            let receiver = receiver.as_slice();
            sqlx::query!(
                "INSERT INTO mls_message_receivers (message_id, spki_hash) VALUES (?,?)",
                message_id,
                receiver
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(())
    }

    // pub async fn aknowledge_messages(
    //     &self,
    //     receiver: &SpkiHash,
    //     messages: &[Uuid],
    // ) -> Result<(), MessageStoreError> {
    //     let mut tx = self.pool.begin().await?;
    //     let receiver = receiver.as_slice();

    //     for message in messages {
    //         sqlx::query!(
    //             "DELETE FROM mls_message_receivers WHERE message_id = ? AND spki_hash = ?",
    //             message,
    //             receiver
    //         )
    //         .execute(&mut *tx)
    //         .await?;

    //         sqlx::query!("DELETE FROM mls_messages WHERE id = ? AND NOT EXISTS ( SELECT 1 FROM mls_message_receivers WHERE message_id = ? ) ", message, message).execute(&mut *tx).await?;
    //     }

    //     Ok(())
    // }
}

#[derive(Debug, thiserror::Error)]
pub enum MessageStoreError {
    #[error("db error: {0}")]
    DBError(#[from] sqlx::Error),
    #[error("postcard error: {0}")]
    PostcardError(#[from] postcard::Error),
}
