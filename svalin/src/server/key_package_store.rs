use std::{collections::HashSet, sync::Arc};

use svalin_pki::{
    SpkiHash,
    mls::key_package::{KeyPackage, UnverifiedKeyPackage},
};

#[derive(Debug)]
pub struct KeyPackageStore {
    pool: sqlx::SqlitePool,
}

impl KeyPackageStore {
    pub(crate) fn open(pool: sqlx::Pool<sqlx::Sqlite>) -> Arc<Self> {
        Arc::new(Self { pool })
    }

    pub async fn add_key_package(&self, key_package: KeyPackage) -> anyhow::Result<()> {
        let spki_hash = key_package.spki_hash().clone();
        let spki_hash = spki_hash.as_slice();
        let member = key_package.to_unverified();
        let data = postcard::to_stdvec(&member)?;
        let id = uuid::Uuid::new_v4().as_hyphenated().to_string();

        sqlx::query!(
            "INSERT INTO key_packages (id, spki_hash, data) VALUES (?, ?, ?)",
            id,
            spki_hash,
            data
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn count_key_packages(&self, spki_hash: &SpkiHash) -> anyhow::Result<u64> {
        let spki_hash = spki_hash.as_slice();
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) as count FROM key_packages WHERE spki_hash = ?",
            spki_hash
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count as u64)
    }

    pub async fn get_key_packages(
        &self,
        entities: &HashSet<SpkiHash>,
    ) -> anyhow::Result<Vec<UnverifiedKeyPackage>> {
        let mut transaction = self.pool.begin().await?;

        let mut key_packages = Vec::new();

        for spki_hash in entities.iter() {
            let spki_hash = spki_hash.as_slice();
            let record = sqlx::query!(
                "SELECT id, data FROM key_packages WHERE spki_hash = ? LIMIT 1",
                spki_hash
            )
            .fetch_one(&mut *transaction)
            .await?;
            sqlx::query!("DELETE FROM key_packages WHERE id = ?", record.id)
                .execute(&mut *transaction)
                .await?;
            let member = postcard::from_bytes(&record.data)?;
            key_packages.push(member);
        }

        if key_packages.len() < entities.len() {
            transaction.rollback().await?;
            return Err(anyhow::anyhow!("Not all key packages found"));
        }

        transaction.commit().await?;

        Ok(key_packages)
    }
}
