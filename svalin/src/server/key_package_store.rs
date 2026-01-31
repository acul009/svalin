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
        let owner_spki_hash = key_package.spki_hash().clone();
        let owner_spki_hash = owner_spki_hash.as_slice();
        let user_spki_hash = key_package.user_spki_hash().clone();
        let user_spki_hash = user_spki_hash.as_slice();
        let member = key_package.to_unverified();
        let data = postcard::to_stdvec(&member)?;
        let id = uuid::Uuid::new_v4().as_hyphenated().to_string();

        sqlx::query!(
            "INSERT INTO key_packages (id, owner_spki_hash, user_spki_hash, data) VALUES (?, ?, ?, ?)",
            id,
            owner_spki_hash,
            user_spki_hash,
            data
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn count_key_packages(&self, owner: &SpkiHash) -> anyhow::Result<u64> {
        let spki_hash = owner.as_slice();
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) as count FROM key_packages WHERE owner_spki_hash = ?",
            spki_hash
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count as u64)
    }

    pub async fn get_key_packages(
        &self,
        entities: &HashSet<SpkiHash>,
        ignore: &SpkiHash,
    ) -> anyhow::Result<Vec<UnverifiedKeyPackage>> {
        let mut transaction = self.pool.begin().await?;

        let mut key_packages = Vec::new();
        let ignore = ignore.as_slice();

        for spki_hash in entities.iter() {
            let spki_hash = spki_hash.as_slice();
            let user_key_packages = sqlx::query!(
                "SELECT id, data FROM key_packages WHERE user_spki_hash = ? AND owner_spki_hash != ? GROUP BY owner_spki_hash",
                spki_hash,
                ignore
            ).fetch_all(&mut *transaction).await?;
            for key_package in user_key_packages {
                sqlx::query!("DELETE FROM key_packages WHERE id = ?", key_package.id)
                    .execute(&mut *transaction)
                    .await?;
                let member = postcard::from_bytes(&key_package.data)?;
                key_packages.push(member);
            }
        }

        tracing::debug!("requested: {:#?}", entities);
        tracing::debug!("found: {:#?}", key_packages);

        transaction.commit().await?;

        Ok(key_packages)
    }
}
