use svalin_pki::{SpkiHash, mls::NewMember};

#[derive(Debug)]
pub struct KeyPackageStore {
    pool: sqlx::SqlitePool,
}

impl KeyPackageStore {
    pub async fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn add_key_packages(
        &self,
        entity: &SpkiHash,
        key_packages: Vec<NewMember>,
    ) -> anyhow::Result<()> {
        todo!()
    }

    pub async fn get_key_packages(
        &self,
        entities: Vec<SpkiHash>,
    ) -> anyhow::Result<Vec<KeyPackage>> {
        todo!()
    }
}
