use openmls_sqlx_storage::SqliteStorageProvider;
use sqlx::SqlitePool;

use crate::{Credential, mls::client::PostcardCodec};

#[tokio::test(flavor = "multi_thread")]
async fn test_key_package_creation() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let mut storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    storage.run_migrations().unwrap();
    let credential = Credential::generate_root().unwrap();
    let client = crate::mls::client::MlsClient::new(credential, storage);
    let key_package = client.create_key_package().unwrap();
}
