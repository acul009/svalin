use std::collections::HashSet;

use svalin_pki::{
    SpkiHash,
    mls::new_member::{NewMember, UnverifiedNewMember},
};

#[derive(Debug)]
pub struct KeyPackageStore {
    pool: sqlx::SqlitePool,
}

impl KeyPackageStore {
    pub async fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn add_key_package(&self, member: NewMember) -> anyhow::Result<()> {
        let spki_hash = member.spki_hash().clone();
        let spki_hash = spki_hash.as_slice();
        let member = member.to_unverified();
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

    pub async fn get_key_packages(
        &self,
        entities: &HashSet<SpkiHash>,
    ) -> anyhow::Result<Vec<UnverifiedNewMember>> {
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
