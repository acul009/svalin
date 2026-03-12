use openmls_sqlx_storage::SqliteStorageProvider;
use sqlx::SqlitePool;

use crate::{
    Credential, KeyPair,
    mls::{
        agent::MlsAgent,
        client::MlsClient,
        delivery_service::{self, DeliveryService},
        provider::PostcardCodec,
    },
};

#[tokio::test(flavor = "multi_thread")]
async fn test_key_package_creation() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    storage.run_migrations().await.unwrap();
    let credential = Credential::generate_root().unwrap();
    let client = crate::mls::client::MlsClient::new(credential, storage);
    let key_package = client.create_key_package().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_device_group() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    storage.run_migrations().await.unwrap();
    let user_credential = Credential::generate_root().unwrap();
    let client_credential = user_credential.create_user_device_credential().unwrap();
    let client = crate::mls::client::MlsClient::new(client_credential.clone(), storage);

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool.clone());
    storage.run_migrations().await.unwrap();
    let keypair = KeyPair::generate();
    let public_key = keypair.export_public_key();
    let cert = user_credential
        .create_agent_certificate_for_key(&public_key)
        .unwrap();
    let agent_credential = keypair.upgrade(cert.to_unverified()).unwrap();
    let agent = crate::mls::client::MlsClient::new(agent_credential.clone(), storage);

    let key_package = agent.create_key_package().await.unwrap();

    let info = client
        .create_device_group(key_package, Vec::new())
        .await
        .unwrap();

    agent.join_my_device_group(info.clone()).await.unwrap();

    drop(agent);
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    let agent = crate::mls::agent::MlsAgent::new(agent_credential.clone(), storage)
        .await
        .unwrap();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool.clone());
    storage.run_migrations().await.unwrap();
    let delivery_service = DeliveryService::new(storage);

    delivery_service.new_device_group(info).await.unwrap();

    let report = "Test Data".to_string();
    let encoded = agent.create_new_report(&report).await.unwrap();

    let receivers = delivery_service
        .process_device_group_message(
            agent_credential.get_certificate().spki_hash(),
            encoded.raw().as_slice(),
        )
        .await
        .unwrap()
        .into_iter()
        .filter(|member| member != agent_credential.get_certificate().spki_hash())
        .collect::<Vec<_>>();

    assert_eq!(receivers.len(), 1);
    assert_eq!(
        &receivers[0],
        client_credential.get_certificate().spki_hash()
    );

    client.decode_system_report(agent_credential.get_certificate().spki_hash(), encoded);

    // compile_error!("client now needs to decode this value")
}
