use openmls::{
    group::{MlsGroup, MlsGroupCreateConfig, MlsGroupJoinConfig, StagedWelcome},
    prelude::{
        BasicCredential, Ciphersuite, CredentialWithKey, DeserializeBytes, Extension,
        ExtensionType, Extensions, KeyPackage, MlsMessageIn, OpenMlsProvider, RatchetTreeExtension,
        SenderRatchetConfiguration, SignedStruct,
        tls_codec::{Deserialize, Serialize},
    },
    test_utils::{OpenMlsRustCrypto, test_framework::Group},
    treesync::RatchetTree,
};

use crate::Credential;

#[test]
fn experimenting() {
    // ChaCha20 icompatible with rust crypto
    let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

    // This builds a whole provider, not just the crypto provider.
    // It uses an in-memory DB.
    let provider1 = OpenMlsRustCrypto::default();
    let provider2 = OpenMlsRustCrypto::default();

    let credential1 = Credential::generate_root().unwrap();
    let credential2 = Credential::generate_root().unwrap();

    let client1 = CredentialWithKey {
        credential: credential1.get_certificate().into(),
        signature_key: credential1.get_certificate().public_key().into(),
    };

    let client2 = CredentialWithKey {
        credential: credential2.get_certificate().into(),
        signature_key: credential2.get_certificate().public_key().into(),
    };

    let key_package_1 = KeyPackage::builder()
        .build(ciphersuite, &provider1, &credential1, client1.clone())
        .unwrap();
    let key_package_2 = KeyPackage::builder()
        .build(ciphersuite, &provider2, &credential2, client2.clone())
        .unwrap();

    let key_package_2_serialized = serde_json::to_vec(key_package_2.key_package()).unwrap();
    let key_package_2_copy: KeyPackage = serde_json::from_slice(&key_package_2_serialized).unwrap();

    let sender_ratchet_config = SenderRatchetConfiguration::new(0, 1);

    let group_create_config = MlsGroupCreateConfig::builder()
        .use_ratchet_tree_extension(true)
        .sender_ratchet_configuration(sender_ratchet_config.clone())
        .build();

    let mut group1 = MlsGroup::new(
        &provider1,
        &credential1,
        &group_create_config,
        client1.clone(),
    )
    .unwrap();

    let (mls_message_out, welcome_out, group_info) = group1
        .add_members(&provider1, &credential1, &[key_package_2_copy])
        .unwrap();

    group1.merge_pending_commit(&provider1).unwrap();

    let serialized_invite = welcome_out.tls_serialize_detached().unwrap();

    let received_mls_message =
        MlsMessageIn::tls_deserialize_exact(&mut serialized_invite.as_slice()).unwrap();

    let invite = match received_mls_message.extract() {
        openmls::prelude::MlsMessageBodyIn::Welcome(invite) => invite,
        _ => unreachable!("has to be an invite"),
    };

    let join_config = MlsGroupJoinConfig::builder()
        .use_ratchet_tree_extension(true)
        .sender_ratchet_configuration(sender_ratchet_config)
        .build();

    let staged_join =
        StagedWelcome::new_from_welcome(&provider2, &join_config, invite, None).unwrap();

    let mut group2 = staged_join.into_group(&provider2).unwrap();

    let content1 = b"This is the first test message!";

    let message1 = group1
        .create_message(&provider1, &credential1, content1)
        .unwrap()
        .tls_serialize_detached()
        .unwrap();

    let received_mls_message = MlsMessageIn::tls_deserialize_exact(&mut message1.as_slice())
        .unwrap()
        .try_into_protocol_message()
        .unwrap();

    let processed_message = group2
        .process_message(&provider2, received_mls_message)
        .unwrap();

    let cleartext = match processed_message.into_content() {
        openmls::prelude::ProcessedMessageContent::ApplicationMessage(message) => {
            message.into_bytes()
        }
        _ => unreachable!("message type is controlled here"),
    };

    assert_eq!(content1.as_ref(), &cleartext);
}
