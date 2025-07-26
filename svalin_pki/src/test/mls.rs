use openmls::{
    group::{MlsGroup, MlsGroupCreateConfig, MlsGroupJoinConfig, StagedWelcome},
    prelude::{
        Ciphersuite, CredentialWithKey, DeserializeBytes, KeyPackage, MlsMessageIn,
        OpenMlsProvider, OpenMlsSignaturePublicKey, SenderRatchetConfiguration, SignaturePublicKey,
        Verifiable,
        tls_codec::{Deserialize, Serialize},
    },
    test_utils::OpenMlsRustCrypto,
};

use crate::{Certificate, Credential};

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

    let invite = received_mls_message
        .into_welcome()
        .expect("Has to be an invite");

    for secret in invite.secrets() {
        let new_member = secret.new_member();
    }

    let join_config = MlsGroupJoinConfig::builder()
        .use_ratchet_tree_extension(true)
        .sender_ratchet_configuration(sender_ratchet_config)
        .build();

    let staged_join =
        StagedWelcome::new_from_welcome(&provider2, &join_config, invite, None).unwrap();

    // You can iterate over all members and grab their certificates...
    for member in staged_join.members() {
        let cert: Certificate = member.credential.deserialized().unwrap();
        println!("{}", cert.spki_hash());
    }

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

    let sender: Certificate = processed_message.credential().deserialized().unwrap();
    println!("sent by: {}", sender.spki_hash());

    let cleartext = match processed_message.into_content() {
        openmls::prelude::ProcessedMessageContent::ApplicationMessage(message) => {
            message.into_bytes()
        }
        _ => unreachable!("message type is controlled here"),
    };

    assert_eq!(content1.as_ref(), &cleartext);

    let serialized_group_info = group1
        .export_group_info(provider1.crypto(), &credential1, false)
        .unwrap()
        .tls_serialize_detached()
        .unwrap();

    let verifyable_group_info = MlsMessageIn::tls_deserialize_exact_bytes(&serialized_group_info)
        .unwrap()
        .into_verifiable_group_info()
        .unwrap();

    let group_info = verifyable_group_info
        .verify(
            provider1.crypto(),
            &OpenMlsSignaturePublicKey::from_signature_key(
                SignaturePublicKey::from(credential1.get_certificate().public_key()),
                openmls::prelude::SignatureScheme::ED25519,
            ),
        )
        .unwrap();
}
