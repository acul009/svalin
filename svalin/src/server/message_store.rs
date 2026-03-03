#[derive(Debug)]
pub struct MessageStore {
    pool: sqlx::SqlitePool,
}
