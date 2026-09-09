use futures::StreamExt;
use sqlx::SqlitePool;
use std::{fmt::Debug, path::Path, sync::Arc};
use svalin_pki::SpkiHash;

use crate::client::state::persistent;

use super::{close_handle::CloseHandle, trust_store_transaction_store::TrustStoreTransactionStore};

pub struct ClientStore {
    pool: SqlitePool,
    transaction_store: Arc<TrustStoreTransactionStore>,
}

impl ClientStore {
    pub async fn open(filename: impl AsRef<Path>) -> Result<Self, Error> {
        let pool = super::open_database(filename).await?;

        Ok(Self {
            transaction_store: Arc::new(TrustStoreTransactionStore::open(pool.clone()).await?),
            pool,
        })
    }

    pub async fn update(&self, message: &persistent::Update) -> Result<(), Error> {
        match &message {
            persistent::Update::SystemReport(spki_hash, system_report) => {
                let report = rmp_serde::to_vec_named(system_report)?;
                let spki_hash = spki_hash.as_slice();
                let generated_at = system_report.system_report.generated_at as i64;

                sqlx::query!("INSERT INTO system_reports (spki_hash, report, generated_at) VALUES (?, ?, ?) ON CONFLICT(spki_hash) DO UPDATE SET report = ?, generated_at = ? where generated_at < ?", spki_hash, report, generated_at, report, generated_at, generated_at)
                    .execute(&self.pool)
                    .await?;
            }
            persistent::Update::MetaInfo(spki_hash, meta_info) => {
                let info = rmp_serde::to_vec_named(meta_info)?;
                let spki_hash = spki_hash.as_slice();
                let updated_at = meta_info.updated_at as i64;

                sqlx::query!("INSERT INTO meta_info (spki_hash, data, updated_at) VALUES (?, ?, ?) ON CONFLICT(spki_hash) DO UPDATE SET data = ?, updated_at = ? where updated_at < ?", spki_hash, info, updated_at, info, updated_at, updated_at)
                    .execute(&self.pool)
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn load_persistent(&self) -> Result<persistent::State, Error> {
        let mut reports =
            sqlx::query!(r#"SELECT spki_hash as "spki_hash!", report FROM system_reports"#)
                .fetch(&self.pool);

        let mut state = persistent::State::empty();

        while let Some(row) = reports.next().await {
            let row = row?;
            let spki_hash = SpkiHash::from_slice(&row.spki_hash)
                .expect("values should have been checked when saving in the db");
            match rmp_serde::from_slice(&row.report) {
                Ok(report) => state.update(persistent::Update::SystemReport(spki_hash, report)),
                Err(err) => tracing::error!("failed to load report: {}", err),
            }
        }

        let mut meta_info =
            sqlx::query!(r#"SELECT spki_hash as "spki_hash!", data FROM meta_info"#)
                .fetch(&self.pool);

        while let Some(row) = meta_info.next().await {
            let row = row?;
            let spki_hash = SpkiHash::from_slice(&row.spki_hash)
                .expect("values should have been checked when saving in the db");
            match rmp_serde::from_slice(&row.data) {
                Ok(data) => state.update(persistent::Update::MetaInfo(spki_hash, data)),
                Err(err) => tracing::error!("failed to load meta info: {}", err),
            }
        }

        Ok(state)
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
