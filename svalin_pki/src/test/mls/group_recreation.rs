use super::*;

#[tokio::test]
async fn recreates_existing_device_group() {
    let mut verifier = TestVerifier::new();
    let retriever = TestRetriever::new();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let client_storage = SqliteStorageProvider::<MessagePackCodec>::new(pool);
    client_storage.run_migrations().await.unwrap();
    let user_credential = Credential::generate_root().unwrap();
    verifier.push(user_credential.certificate().clone());

    let client_credential = user_credential.create_user_session_credential().unwrap();
    verifier.push(client_credential.certificate().clone());

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let agent_storage = SqliteStorageProvider::<MessagePackCodec>::new(pool.clone());
    agent_storage.run_migrations().await.unwrap();
    let keypair = KeyPair::generate();
    let public_key = keypair.export_public_key();
    let cert = user_credential
        .create_agent_certificate_for_key(&public_key)
        .unwrap();
    let agent_credential = keypair.upgrade(cert.to_unverified()).unwrap();
    verifier.push(agent_credential.certificate().clone());

    let client = MlsClient::<Types, _, _>::new(
        client_credential.clone(),
        client_storage.into(),
        retriever.clone(),
        verifier.clone(),
    )
    .unwrap();
    retriever.add(client.create_key_package().await.unwrap());

    let agent = MlsAgent::<Types, _, _>::new(
        agent_credential.clone(),
        agent_storage,
        retriever.clone(),
        verifier.clone(),
    )
    .await
    .unwrap();

    retriever.add(agent.create_key_package().await.unwrap());

    let new_group = agent
        .create_device_group_if_missing()
        .await
        .unwrap()
        .unwrap();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<MessagePackCodec>::new(pool);
    storage.run_migrations().await.unwrap();
    let server = MlsServer::new(storage, verifier.clone(), retriever.clone());
    let welcome = server.process_message(new_group).await.unwrap();

    let joined = client.handle_message(&welcome[0].message).await.unwrap();
    assert!(matches!(
        joined.content,
        MessageDataContent::GroupAdded(group) if group == joined.group
    ));

    let report = "Test Data".to_string();
    let to_server = agent.send_report(report.clone()).await.unwrap();

    let to_send = server.process_message(to_server).await.unwrap();

    let received = client.handle_message(&to_send[0].message).await.unwrap();

    let MessageDataContent::Report(sender, received_report) = received.content else {
        panic!("wrong message type")
    };

    assert_eq!(&sender, agent_credential.certificate().spki_hash());
    assert_eq!(&received_report, &report);

    // Supply a fresh package for the client, then replace the existing group
    // without restarting any of the processors (the server cache is populated).
    retriever.add(client.create_key_package().await.unwrap());
    let replacement = agent.recreate_device_group().await.unwrap();
    let welcome = server.process_message(replacement).await.unwrap();
    let rejoined = client.handle_message(&welcome[0].message).await.unwrap();
    assert_eq!(rejoined.group, joined.group);
    assert!(matches!(
        rejoined.content,
        MessageDataContent::GroupAdded(_)
    ));

    let report = "Report after recreation".to_string();
    let message = agent.send_report(report.clone()).await.unwrap();
    let forwarded = server.process_message(message).await.unwrap();
    let received = client.handle_message(&forwarded[0].message).await.unwrap();
    let MessageDataContent::Report(sender, received_report) = received.content else {
        panic!("expected report from recreated group");
    };
    assert_eq!(&sender, agent_credential.certificate().spki_hash());
    assert_eq!(received_report, report);
}
