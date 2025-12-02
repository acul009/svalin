#[derive(Debug)]
pub struct KeyPackageStore {
    pool: sqlx::SqlitePool,
}
