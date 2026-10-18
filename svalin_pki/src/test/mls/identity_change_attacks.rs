use super::*;

// Both identities are cryptographically valid, but an update must not replace a required member.
#[tokio::test]
async fn required_member_cannot_change_identity_via_update() {
    use crate::mls::{SvalinGroupId, harness::MlsHarness, provider::SvalinProvider};
    use openmls::prelude::{
        CredentialWithKey, LeafNodeParameters, MlsGroup, MlsMessageBodyOut,
        PURE_PLAINTEXT_WIRE_FORMAT_POLICY,
    };
    let owner = Credential::generate_root().unwrap();
    let victim = Credential::generate_root().unwrap();
    let peer = create_processor(Credential::generate_root().unwrap()).await;
    let mut verifier = TestVerifier::new();
    verifier.push(owner.certificate().clone());
    verifier.push(victim.certificate().clone());
    let retriever = TestRetriever::new();
    let owner_processor = create_processor(owner.clone()).await;
    retriever.add(owner_processor.create_key_package().await.unwrap());
    let harness = MlsHarness::new(
        retriever.clone(),
        verifier.clone(),
        create_public_processor().await,
    );
    let provider = SvalinProvider::new(openmls_rust_crypto::MemoryStorage::default());
    let id = SvalinGroupId::DeviceMetaGroup(owner.certificate().spki_hash().clone());
    let mut group = MlsGroup::builder()
        .ciphersuite(provider.ciphersuite())
        .with_group_id(id.to_group_id())
        .use_ratchet_tree_extension(true)
        .with_wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
        .build(
            &provider,
            &owner,
            CredentialWithKey {
                credential: owner.certificate().spki_hash().into(),
                signature_key: owner.certificate().public_key().into(),
            },
        )
        .unwrap();
    let (_, welcome, info) = group
        .add_members(
            &provider,
            &owner,
            &[peer.create_key_package().await.unwrap().unpack()],
        )
        .unwrap();
    group.merge_pending_commit(&provider).unwrap();
    let MlsMessageBodyOut::Welcome(welcome) = welcome.body() else {
        panic!("expected Welcome");
    };
    let MessageToServer::NewDeviceGroup(initial) =
        crate::mls::transport_types::MessageToServerTransport::new_device_group(
            &info.unwrap(),
            Some(welcome),
        )
        .unwrap()
        .unpack()
        .unwrap()
    else {
        panic!("expected new group")
    };
    harness.processor().add_group(initial).await.unwrap();
    let staged = peer.stage_join(welcome.clone()).await.unwrap();
    peer.join_group(staged).await.unwrap();
    let member_harness = MlsHarness::new(retriever, verifier, peer);

    let commit = group
        .self_update_with_new_signer(
            &provider,
            &owner,
            openmls::credentials::NewSignerBundle {
                signer: &victim,
                credential_with_key: CredentialWithKey {
                    credential: victim.certificate().spki_hash().into(),
                    signature_key: victim.certificate().public_key().into(),
                },
            },
            LeafNodeParameters::default(),
        )
        .unwrap()
        .into_contents()
        .0;
    let encoded = openmls::prelude::tls_codec::Serialize::tls_serialize_detached(&commit).unwrap();
    let MessageToMember::AddToGroup(commit) = MessageToMemberTransport::AddToGroup(encoded)
        .unpack()
        .unwrap()
    else {
        panic!("expected commit")
    };
    let member_processed = member_harness
        .processor()
        .process_message(commit.clone())
        .await
        .unwrap();
    let ProcessedContent::Commit(member_commit) = member_processed.content else {
        panic!("expected member staged commit");
    };
    let error = member_harness
        .check_commit(&id, &member_commit, &member_processed.sender)
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "Updates cannot change member identity");

    let processed = harness
        .processor()
        .process_message(commit.into())
        .await
        .unwrap();
    let crate::mls::public_processor::ProcessedContent::Commit { commit, sender } =
        processed.content
    else {
        panic!("expected staged commit")
    };

    assert_eq!(commit.remove_proposals().count(), 0);
    assert_eq!(
        commit
            .update_path_leaf_node()
            .unwrap()
            .credential()
            .deserialized::<SpkiHash>()
            .unwrap(),
        *victim.certificate().spki_hash(),
    );
    let error = harness
        .check_commit(&id, &commit, &sender)
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "Updates cannot change member identity");
}
