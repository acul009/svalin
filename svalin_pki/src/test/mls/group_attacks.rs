use super::*;

#[tokio::test]
async fn test_rejects_forged_groups() {
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

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<MessagePackCodec>::new(pool);
    storage.run_migrations().await.unwrap();
    let server = MlsServer::new(storage, verifier.clone(), retriever.clone());

    // OpenMLS accepts this tree's signatures, but its creator claims the root
    // identity while using an unrelated signing key. Reject it on both import
    // and Welcome acceptance, before persisting the group.
    let attacker = Credential::generate_root().unwrap();
    let provider =
        crate::mls::provider::SvalinProvider::new(openmls_rust_crypto::MemoryStorage::default());
    let mut forged = openmls::prelude::MlsGroup::builder()
        .ciphersuite(provider.ciphersuite())
        .with_group_id(
            crate::mls::SvalinGroupId::DeviceMetaGroup(
                agent_credential.certificate().spki_hash().clone(),
            )
            .to_group_id(),
        )
        .use_ratchet_tree_extension(true)
        .build(
            &provider,
            &attacker,
            openmls::prelude::CredentialWithKey {
                credential: user_credential.certificate().spki_hash().into(),
                signature_key: attacker.certificate().public_key().into(),
            },
        )
        .unwrap();
    let (_, welcome, info) = forged
        .add_members(
            &provider,
            &attacker,
            &[client.create_key_package().await.unwrap().unpack()],
        )
        .unwrap();
    let openmls::prelude::MlsMessageBodyOut::Welcome(welcome) = welcome.body() else {
        panic!("expected Welcome");
    };
    let forged_message = crate::mls::transport_types::MessageToServerTransport::new_device_group(
        &info.unwrap(),
        Some(welcome),
    )
    .unwrap();
    let encoded = rmp_serde::to_vec_named(&forged_message).unwrap();
    let MessageToServer::NewDeviceGroup(forged_group) = forged_message.unpack().unwrap() else {
        panic!("expected new group");
    };
    // Ensure failures below come from our identity checks, not MLS signatures.
    create_public_processor()
        .await
        .check_group(forged_group.clone())
        .await
        .unwrap();
    let member_message = MessageToMemberTransport::Welcome(forged_group.welcome.clone().unwrap());
    let err = server
        .process_message(rmp_serde::from_slice(&encoded).unwrap())
        .await
        .unwrap_err();
    assert!(matches!(
        err.downcast_ref::<crate::mls::server::AddDeviceGroupError<Never>>(),
        Some(
            crate::mls::server::AddDeviceGroupError::IdentityVerification(
                crate::mls::harness::IdentityVerificationError::SignatureKeyMismatch(_)
            )
        )
    ));
    assert!(matches!(
        client.handle_message(&member_message).await,
        Err(crate::mls::client::HandleMessageError::Welcome(
            crate::mls::client::HandleWelcomeError::IdentityVerification(
                crate::mls::harness::IdentityVerificationError::SignatureKeyMismatch(_)
            )
        ))
    ));
    // A group with the required members must still be signed by the device.
    let other_creator = create_processor(client_credential.clone()).await;
    let incorrectly_signed = other_creator
        .create_group(
            vec![agent.create_key_package().await.unwrap()],
            crate::mls::SvalinGroupId::DeviceGroup(
                agent_credential.certificate().spki_hash().clone(),
            )
            .to_group_id(),
        )
        .await
        .unwrap();
    let err = server
        .process_message(incorrectly_signed)
        .await
        .unwrap_err();
    assert!(matches!(
        err.downcast_ref::<crate::mls::server::AddDeviceGroupError<Never>>(),
        Some(crate::mls::server::AddDeviceGroupError::UnauthorizedCreator)
    ));
}
