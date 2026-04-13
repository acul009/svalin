use std::sync::Arc;

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
        let spki_hash_slice = spki_hash.as_slice();
        let member = key_package.to_unverified();
        if member.spki_hash()? != spki_hash {
            anyhow::bail!("Key package hash mismatch");
        }

        let data = postcard::to_stdvec(&member)?;
        let id = uuid::Uuid::new_v4().as_hyphenated().to_string();

        sqlx::query!(
            "INSERT INTO key_packages (id, spki_hash, data) VALUES (?, ?, ?)",
            id,
            spki_hash_slice,
            data
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn count_key_packages(&self, owner: &SpkiHash) -> anyhow::Result<u64> {
        let spki_hash = owner.as_slice();
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
        entities: impl ExactSizeIterator<Item = &SpkiHash>,
    ) -> anyhow::Result<Vec<UnverifiedKeyPackage>> {
        let mut transaction = self.pool.begin().await?;

        let mut key_packages = Vec::with_capacity(entities.len());

        for spki_hash in entities {
            // tracing::debug!("Loading key package for {spki_hash}");
            let spki_hash_slice = spki_hash.as_slice();
            let key_package = sqlx::query!(
                "SELECT id, data FROM key_packages WHERE spki_hash == ? LIMIT 1",
                spki_hash_slice,
            )
            .fetch_one(&mut *transaction)
            .await?;

            sqlx::query!("DELETE FROM key_packages WHERE id = ?", key_package.id)
                .execute(&mut *transaction)
                .await?;
            let key_package: UnverifiedKeyPackage = postcard::from_bytes(&key_package.data)?;
            if &key_package.spki_hash()? != spki_hash {
                anyhow::bail!("Key package in store does not match the expected owner");
            }
            key_packages.push(key_package);
        }

        transaction.commit().await?;

        Ok(key_packages)
    }
}
