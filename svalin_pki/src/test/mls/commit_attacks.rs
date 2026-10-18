use super::*;
use crate::mls::{SvalinGroupId, harness::MlsHarness, provider::SvalinProvider};
use openmls::prelude::{
    CredentialWithKey, LeafNodeParameters, MlsGroup, MlsMessageBodyOut, MlsMessageOut,
    PURE_PLAINTEXT_WIRE_FORMAT_POLICY,
};

struct CommitFixture {
    owner: Credential,
    victim: Credential,
    provider: SvalinProvider,
    group: MlsGroup,
    id: SvalinGroupId,
    harness: MlsHarness<TestRetriever, TestVerifier, PublicProcessorHandle>,
}

impl CommitFixture {
    async fn new() -> Self {
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

        Self {
            owner,
            victim,
            provider,
            group,
            id,
            harness,
        }
    }

    async fn check_commit(&self, commit: MlsMessageOut) -> anyhow::Result<()> {
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
        let processed = self
            .harness
            .processor()
            .process_message(commit.into())
            .await
            .unwrap();
        let crate::mls::public_processor::ProcessedContent::Commit { commit, sender } =
            processed.content
        else {
            panic!("expected staged commit")
        };
        self.harness.check_commit(&self.id, &commit, &sender).await
    }
}

#[tokio::test]
async fn accepts_valid_update() {
    let mut fixture = CommitFixture::new().await;
    let commit = fixture
        .group
        .self_update(
            &fixture.provider,
            &fixture.owner,
            LeafNodeParameters::default(),
        )
        .unwrap()
        .into_contents()
        .0;
    fixture.check_commit(commit).await.unwrap();
}

#[tokio::test]
async fn rejects_update_with_forged_identity() {
    let mut fixture = CommitFixture::new().await;
    let parameters = LeafNodeParameters::builder()
        .with_credential_with_key(CredentialWithKey {
            credential: fixture.victim.certificate().spki_hash().into(),
            signature_key: fixture.owner.certificate().public_key().into(),
        })
        .build();
    let commit = fixture
        .group
        .self_update(&fixture.provider, &fixture.owner, parameters)
        .unwrap()
        .into_contents()
        .0;
    let error = fixture.check_commit(commit).await.unwrap_err();
    assert!(matches!(
        error.downcast_ref::<crate::mls::harness::IdentityVerificationError>(),
        Some(crate::mls::harness::IdentityVerificationError::SignatureKeyMismatch(_))
    ));
}

#[tokio::test]
async fn rejects_add_with_forged_identity() {
    let mut fixture = CommitFixture::new().await;
    let outsider = Credential::generate_root().unwrap();
    let package = openmls::prelude::KeyPackage::builder()
        .build(
            fixture.provider.ciphersuite(),
            &fixture.provider,
            &outsider,
            CredentialWithKey {
                credential: fixture.victim.certificate().spki_hash().into(),
                signature_key: outsider.certificate().public_key().into(),
            },
        )
        .unwrap();
    let commit = fixture
        .group
        .add_members(
            &fixture.provider,
            &fixture.owner,
            &[package.key_package().clone()],
        )
        .unwrap()
        .0;
    let error = fixture.check_commit(commit).await.unwrap_err();
    assert!(matches!(
        error.downcast_ref::<crate::mls::key_package::KeyPackageError>(),
        Some(crate::mls::key_package::KeyPackageError::SignatureKeyMismatch)
    ));
}
