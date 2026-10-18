use super::*;

#[tokio::test]
async fn test_commit_identity_binding() {
    use crate::mls::{SvalinGroupId, harness::MlsHarness, provider::SvalinProvider};
    use openmls::prelude::{
        CredentialWithKey, LeafNodeParameters, MlsGroup, MlsMessageBodyOut,
        PURE_PLAINTEXT_WIRE_FORMAT_POLICY,
    };

    for attack in [None, Some("update"), Some("add")] {
        let owner = Credential::generate_root().unwrap();
        let victim = Credential::generate_root().unwrap();
        let peer = create_processor(Credential::generate_root().unwrap()).await;
        let mut verifier = TestVerifier::new();
        verifier.push(owner.certificate().clone());
        verifier.push(victim.certificate().clone());
        let harness = MlsHarness::new(
            TestRetriever::new(),
            verifier,
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

        let forged_identity = CredentialWithKey {
            credential: victim.certificate().spki_hash().into(),
            signature_key: owner.certificate().public_key().into(),
        };
        let commit = if attack == Some("add") {
            let outsider = Credential::generate_root().unwrap();
            let package = openmls::prelude::KeyPackage::builder()
                .build(
                    provider.ciphersuite(),
                    &provider,
                    &outsider,
                    CredentialWithKey {
                        credential: victim.certificate().spki_hash().into(),
                        signature_key: outsider.certificate().public_key().into(),
                    },
                )
                .unwrap();
            group
                .add_members(&provider, &owner, &[package.key_package().clone()])
                .unwrap()
                .0
        } else {
            let parameters = if attack.is_some() {
                LeafNodeParameters::builder()
                    .with_credential_with_key(forged_identity)
                    .build()
            } else {
                LeafNodeParameters::default()
            };
            group
                .self_update(&provider, &owner, parameters)
                .unwrap()
                .into_contents()
                .0
        };
        // OpenMLS accepts these signatures and stages the commit. Svalin must
        // reject the credential/key mismatch before merging it.
        let encoded =
            openmls::prelude::tls_codec::Serialize::tls_serialize_detached(&commit).unwrap();
        let MessageToMember::AddToGroup(commit) = MessageToMemberTransport::AddToGroup(encoded)
            .unpack()
            .unwrap()
        else {
            panic!("expected commit")
        };
        let processed = harness
            .processor()
            .process_message(commit.into())
            .await
            .unwrap();
        let crate::mls::public_processor::ProcessedContent::Commit(commit) = processed.content
        else {
            panic!("expected staged commit")
        };
        let result = harness.check_commit(&id, &commit).await;
        match attack {
            None => result.unwrap(),
            Some("update") => assert!(matches!(
                result
                    .unwrap_err()
                    .downcast_ref::<crate::mls::harness::IdentityVerificationError>(),
                Some(crate::mls::harness::IdentityVerificationError::SignatureKeyMismatch(_))
            )),
            Some("add") => assert!(matches!(
                result
                    .unwrap_err()
                    .downcast_ref::<crate::mls::key_package::KeyPackageError>(),
                Some(crate::mls::key_package::KeyPackageError::SignatureKeyMismatch)
            )),
            _ => unreachable!(),
        }
    }
}
