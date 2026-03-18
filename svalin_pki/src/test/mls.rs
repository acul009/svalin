use openmls_sqlx_storage::SqliteStorageProvider;
use sqlx::SqlitePool;

use crate::{
    Credential, KeyPair,
    mls::{
        delivery_service::{self, DeliveryServiceHandle},
        processor::MlsProcessorHandle,
        provider::PostcardCodec,
    },
};

async fn create_processor(credential: Credential) -> MlsProcessorHandle {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    storage.run_migrations().await.unwrap();
    let handle = crate::mls::processor::MlsProcessorHandle::new_processor(credential, storage);
    handle
}

async fn create_delivery_service() -> DeliveryServiceHandle {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    storage.run_migrations().await.unwrap();
    let handle = DeliveryServiceHandle::new(storage);
    handle
}

#[tokio::test]
async fn test_key_package_creation() {
    let credential = Credential::generate_root().unwrap();
    let processor = create_processor(credential).await;
    let _key_package = processor.create_key_package().await.unwrap();
}

#[tokio::test]
async fn test_group() {
    let credential1 = Credential::generate_root().unwrap();
    let member1 = create_processor(credential1).await;

    let credential2 = Credential::generate_root().unwrap();
    let member2 = create_processor(credential2).await;
    let key_package = member2.create_key_package().await.unwrap();

    let group_id = b"test".to_vec();

    let group_info = member1
        .create_group(vec![key_package], group_id.clone())
        .await
        .unwrap();

    member2.join_group(group_info).await.unwrap();

    let test_text = b"Hello MLS!".to_vec();

    let message = member2
        .create_message(group_id, test_text.clone())
        .await
        .unwrap();

    let received = member1.process_message(message).await.unwrap();

    assert_eq!(test_text, received);
}

#[tokio::test]
async fn test_delivery_service() {
    let credential1 = Credential::generate_root().unwrap();
    let spki_hash1 = credential1.get_certificate().spki_hash().clone();
    let member1 = create_processor(credential1).await;

    let credential2 = Credential::generate_root().unwrap();
    let spki_hash2 = credential2.get_certificate().spki_hash().clone();
    let member2 = create_processor(credential2).await;
    let key_package = member2.create_key_package().await.unwrap();

    let group_id = b"test".to_vec();

    let group_info = member1
        .create_group(vec![key_package], group_id.clone())
        .await
        .unwrap();

    let delivery_service = create_delivery_service().await;

    let members = delivery_service
        .new_group(group_info.clone())
        .await
        .unwrap();
    let members = members
        .into_iter()
        .filter(|spki_hash| spki_hash != &spki_hash1)
        .collect::<Vec<_>>();

    assert_eq!(members.len(), 1);
    assert_eq!(members[0], spki_hash2);

    member2.join_group(group_info).await.unwrap();

    let test_text = b"Hello MLS!".to_vec();

    let message = member2
        .create_message(group_id, test_text.clone())
        .await
        .unwrap();

    let members = delivery_service
        .process_message(message.clone())
        .await
        .unwrap();
    let members = members
        .into_iter()
        .filter(|spki_hash| spki_hash != &spki_hash2)
        .collect::<Vec<_>>();

    assert_eq!(members.len(), 1);
    assert_eq!(members[0], spki_hash1);

    let received = member1.process_message(message.clone()).await.unwrap();

    assert_eq!(test_text, received);
}

#[tokio::test]
async fn test_device_group() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    storage.run_migrations().await.unwrap();
    let user_credential = Credential::generate_root().unwrap();
    let client_credential = user_credential.create_user_device_credential().unwrap();
    let client = crate::mls::processor::MlsProcessorHandle::new_processor(
        client_credential.clone(),
        storage,
    );

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool.clone());
    storage.run_migrations().await.unwrap();
    let keypair = KeyPair::generate();
    let public_key = keypair.export_public_key();
    let cert = user_credential
        .create_agent_certificate_for_key(&public_key)
        .unwrap();
    let agent_credential = keypair.upgrade(cert.to_unverified()).unwrap();
    let agent =
        crate::mls::processor::MlsProcessorHandle::new_processor(agent_credential.clone(), storage);

    let key_package = agent.create_key_package().await.unwrap();

    // let info = client
    //     .create_device_group(key_package, Vec::new())
    //     .await
    //     .unwrap();

    // agent.join_my_device_group(info.clone()).await.unwrap();

    // drop(agent);
    // let storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    // let agent = crate::mls::agent::MlsAgent::new(agent_credential.clone(), storage)
    //     .await
    //     .unwrap();

    // let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    // let storage = SqliteStorageProvider::<PostcardCodec>::new(pool.clone());
    // storage.run_migrations().await.unwrap();
    // let delivery_service = DeliveryService::new(storage);

    // delivery_service.new_device_group(info).await.unwrap();

    // let report = "Test Data".to_string();
    // let encoded = agent.create_new_report(&report).await.unwrap();

    // let receivers = delivery_service
    //     .process_device_group_message(
    //         agent_credential.get_certificate().spki_hash(),
    //         encoded.raw().as_slice(),
    //     )
    //     .await
    //     .unwrap()
    //     .into_iter()
    //     .filter(|member| member != agent_credential.get_certificate().spki_hash())
    //     .collect::<Vec<_>>();

    // assert_eq!(receivers.len(), 1);
    // assert_eq!(
    //     &receivers[0],
    //     client_credential.get_certificate().spki_hash()
    // );

    // client.decode_system_report(agent_credential.get_certificate().spki_hash(), encoded);

    // compile_error!("client now needs to decode this value")
}
