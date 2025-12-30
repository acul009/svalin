use sqlx::SqlitePool;
use svalin_pki::Certificate;

pub struct RoomManager {
    pool: SqlitePool,
}
