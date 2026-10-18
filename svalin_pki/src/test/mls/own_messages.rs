use super::*;

#[tokio::test]
async fn echoed_private_message_is_internal() {
    let owner = Credential::generate_root().unwrap();
    let agent_key = KeyPair::generate();
    let agent = owner
        .create_agent_certificate_for_key(&agent_key.export_public_key())
        .unwrap();
    let mut verifier = TestVerifier::new();
    verifier.push(owner.certificate().clone());
    verifier.push(agent.clone());
    let client = MlsClient::<Types, _, _>::new(
        owner,
        openmls_rust_crypto::MemoryStorage::default().into(),
        TestRetriever::new(),
        verifier,
    )
    .unwrap();
    client
        .create_meta_group_if_missing(agent.spki_hash().clone())
        .await
        .unwrap()
        .unwrap();
    let MessageToServer::GroupMessage { raw, .. } = client
        .send_meta_info(agent.spki_hash().clone(), "local metadata".into())
        .await
        .unwrap()
        .unpack()
        .unwrap()
    else {
        panic!("expected private message");
    };

    let received = client
        .handle_message(&MessageToMemberTransport::GroupMessage(raw))
        .await
        .unwrap();
    assert_eq!(
        received.group,
        crate::mls::SvalinGroupId::DeviceMetaGroup(agent.spki_hash().clone())
    );
    assert!(matches!(received.content, MessageDataContent::Internal));
}
