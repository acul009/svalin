use futures::StreamExt;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::{fmt::Debug, path::Path};
use svalin_pki::SpkiHash;

use crate::persistent::Message;

mod persistent;

pub struct ClientStore {
    pool: SqlitePool,
}

impl ClientStore {
    pub async fn open(filename: impl AsRef<Path>) -> Result<Self, Error> {
        let options = SqliteConnectOptions::new()
            .create_if_missing(true)
            .filename(filename)
            .optimize_on_close(true, None);

        let pool = SqlitePool::connect_with(options).await?;
        sqlx::migrate!()
            .run(&pool)
            .await
            .map_err(sqlx::Error::from)?;

        Ok(Self { pool })
    }

    pub async fn update(&self, message: &persistent::Message) -> Result<(), Error> {
        match message {
            persistent::Message::UpdateSystemReport(spki_hash, system_report) => {
                let report = postcard::to_stdvec(system_report)?;
                let spki_hash = spki_hash.as_slice();

                sqlx::query!("INSERT INTO system_reports (spki_hash, report) VALUES (?, ?) ON CONFLICT(spki_hash) DO UPDATE SET report = ?", spki_hash, report, report)
                    .execute(&self.pool)
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn load_persistent(&self) -> Result<persistent::ClientState, Error> {
        let mut reports =
            sqlx::query!(r#"SELECT spki_hash as "spki_hash!", report FROM system_reports"#)
                .fetch(&self.pool);

        let mut state = persistent::ClientState::empty();

        while let Some(row) = reports.next().await {
            let row = row?;
            let spki_hash = SpkiHash::from_slice(&row.spki_hash)
                .expect("values should have been checked when saving in the db");
            let report = postcard::from_bytes(&row.report)?;
            state.update(Message::UpdateSystemReport(spki_hash, report));
        }

        todo!()
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}
