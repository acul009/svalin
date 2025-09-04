use std::panic;

use openmls::prelude::{
    KeyPackageIn, MlsMessageBodyIn, MlsMessageIn,
    tls_codec::{Deserialize, Serialize},
};

use crate::{Credential, mls::MlsClient};

#[test]
fn experimenting() {
    // Current state of experiments:
    // MLS would be a nice addition to svalin, but has some issues.
    // Especially the password based login mechanism does not really play well.
    // When a new device is added, it needs access to a lot of groups and the ability
    // to see the latest message in the group (e.g. a new config or a new system report.)
    // For this to work, the storage provider either needs to be shared or there has to be
    // a master store for each user, which would cause even more issues.
    //
    //
    // Then the question is, can a storage provider be shared when 2 devices are online at the same time?
    // I should test what happends when I try to re-read the same group member add 2 times.
    // I'm guessing it's going to cause chaos.
    // So when multiple devices are online at the same time,
    // I need to somehow synchronize which of these devices will update the group state with commits.
    // After a group update, all other devices would need to re-read that group from the storage provider.
    //
    // The only other way I see would be using the external commit system.
    // When sharing the credential, it's easy for a new machine to add itself to all required groups.
    // The problem is, that it then can't read the last message from that group.
    // But in exactly that message is the latest config or system report.
    // So the new device would need help, especially for configs, which don't refresh themselves to see the newest state.
    //
    // I'll still need to think quite a bit about how I could solve this
    // Maybe there's a way to share only parts of the storage provider.

    // ChaCha20 icompatible with rust crypto
    let credential1 = Credential::generate_temporary().unwrap();

    let client1 = MlsClient::new(credential1.clone());
    let client2 = MlsClient::new(Credential::generate_temporary().unwrap());
    let second_device = MlsClient::new(credential1);

    let key_package_2_serialized = client2
        .create_key_package()
        .unwrap()
        .key_package()
        .tls_serialize_detached()
        .unwrap();
    let key_package_second_device_serialized = second_device
        .create_key_package()
        .unwrap()
        .key_package()
        .tls_serialize_detached()
        .unwrap();
    let key_package_2 = KeyPackageIn::tls_deserialize_exact(&key_package_2_serialized).unwrap();
    let key_package_second_device =
        KeyPackageIn::tls_deserialize_exact(&key_package_second_device_serialized).unwrap();

    let mut group1 = client1.create_group().unwrap();

    let (_mls_message, welcome_out) = group1
        .add_members([key_package_2, key_package_second_device])
        .unwrap();

    let serialized_invite = welcome_out.tls_serialize_detached().unwrap();

    let received_mls_message =
        MlsMessageIn::tls_deserialize_exact(&mut serialized_invite.as_slice()).unwrap();

    let welcome = if let MlsMessageBodyIn::Welcome(welcome) = received_mls_message.extract() {
        welcome
    } else {
        panic!("Has to be an invite");
    };

    for secret in welcome.secrets() {
        let new_member = secret.new_member();
    }

    // You can iterate over all members and grab their certificates...
    // for member in staged_join.members() {
    //     let cert: Certificate = member.credential.deserialized().unwrap();
    //     println!("{}", cert.spki_hash());
    // }

    let mut group2 = client2.join_group(welcome.clone()).unwrap();

    let mut group_second_device = second_device.join_group(welcome).unwrap();

    let content1 = b"This is the first test message!";

    let message = group1.create_message(content1).unwrap();

    let cleartext = group2.process_message(message.clone()).unwrap().unwrap();
    let second_cleartext = group_second_device
        .process_message(message)
        .unwrap()
        .unwrap();

    assert_eq!(content1.as_ref(), &cleartext);
    assert_eq!(content1.as_ref(), &second_cleartext);

    // The same message cannot be decrypted again because it's to distant in the past
    // You can however set the out of order tolerance to at least 1 to allow the newest message to be decrypted
    // Or that's what I would say if that didn't trigger a SecretReuseError instead of the ToDistantInThePastError
    // The Problem here is the forward secrecy - a decryption key is dropped the moment a message is decrypted.
    // While that is a good idea in theory, in my case I need to still decrypt the latest message.

    // let received_mls_message = MlsMessageIn::tls_deserialize_exact(&mut message1.as_slice())
    //     .unwrap()
    //     .try_into_protocol_message()
    //     .unwrap();

    // let processed_message = group2
    //     .process_message(client2.provider(), received_mls_message)
    //     .unwrap();

    // let sender: Certificate = processed_message.credential().deserialized().unwrap();
    // println!("sent by: {}", sender.spki_hash());

    // let cleartext = match processed_message.into_content() {
    //     openmls::prelude::ProcessedMessageContent::ApplicationMessage(message) => {
    //         message.into_bytes()
    //     }
    //     _ => unreachable!("message type is controlled here"),
    // };

    // assert_eq!(content1.as_ref(), &cleartext);

    // it seems like the per message ratchet is kept in memory only, so to re-read a message, all you need to do is reload the group from the storage
    // That also seems to have the problematic effect of needing all those messages in the right order again...
    // That can't be right, otherwise a group could not save and load state during an epoch
    //
    // Allright, I figured out how these secrets are handled.
    // They are kept in memory and are serialized together end then stored as one.
    // The store is only triggered on either creating a message myself or merging a staged commit.
    // Which is still weird to me. Does that mean I have to store all messages since the last commit?

    // testing re-reading a newer message
    //
    // let message2 = group1
    //     .create_message(client1.provider(), client1.credential(), content1)
    //     .unwrap()
    //     .tls_serialize_detached()
    //     .unwrap();

    // let mut group2_clone = MlsGroup::load(client2.provider().storage(), group2.group_id())
    //     .unwrap()
    //     .unwrap();

    // let received_mls_message = MlsMessageIn::tls_deserialize_exact(&mut message2.as_slice())
    //     .unwrap()
    //     .try_into_protocol_message()
    //     .unwrap();

    // let processed_message = group2_clone
    //     .process_message(client2.provider(), received_mls_message.clone())
    //     .unwrap();

    // let cleartext = match processed_message.into_content() {
    //     openmls::prelude::ProcessedMessageContent::ApplicationMessage(message) => {
    //         message.into_bytes()
    //     }
    //     _ => unreachable!("message type is controlled here"),
    // };

    // assert_eq!(content1.as_ref(), &cleartext);

    // Notes about re-reading messages:
    //
    // processing messages does not seem to affect the group state.
    // So a message should be able to be read my multiple clients sharing a keystore.
    // The same can probably not be said for messages containing commits
    // Re-reading a message with the same group instance will fail however,
    // as messages received to seem to affect the in memory state of the group.

    // test reading group information

    // let serialized_group_info = group1
    //     .export_group_info(client1.provider().crypto(), client1.credential(), false)
    //     .unwrap()
    //     .tls_serialize_detached()
    //     .unwrap();

    // let verifyable_group_info = MlsMessageIn::tls_deserialize_exact_bytes(&serialized_group_info)
    //     .unwrap()
    //     .into_verifiable_group_info()
    //     .unwrap();

    // let test_group_info = verifyable_group_info
    //     .clone()
    //     .verify(
    //         client1.provider().crypto(),
    //         &OpenMlsSignaturePublicKey::from_signature_key(
    //             SignaturePublicKey::from(client1.credential().get_certificate().public_key()),
    //             openmls::prelude::SignatureScheme::ED25519,
    //         ),
    //     )
    //     .unwrap();

    // let tree = RatchetTreeIn::tls_deserialize_exact_bytes(
    //     &group1
    //         .export_ratchet_tree()
    //         .tls_serialize_detached()
    //         .unwrap(),
    // )
    // .unwrap();
    // let group_info = group_info.unwrap();

    // // group recreation tests
    // let provider3 = OpenMlsRustCrypto::default();
    // let (mut group1_copy, external_join_message, group_info_copy) =
    //     MlsGroup::join_by_external_commit(
    //         &provider3,
    //         client1.credential(),
    //         Some(tree),
    //         verifyable_group_info,
    //         &client1.join_config(),
    //         None,
    //         None,
    //         &[],
    //         client1.public_info().clone(),
    //     )
    //     .unwrap();

    // group1_copy.merge_pending_commit(&provider3).unwrap();

    // let serialized = external_join_message.tls_serialize_detached().unwrap();

    // // device 1 accept external join
    // let message_in = MlsMessageIn::tls_deserialize_exact_bytes(&serialized)
    //     .unwrap()
    //     .into_protocol_message()
    //     .unwrap();

    // let processed = group1
    //     .process_message(client1.provider(), message_in)
    //     .unwrap();
    // let credential = processed.credential().clone();
    // match processed.into_content() {
    //     openmls::prelude::ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
    //         let is_already_member = group1
    //             .members()
    //             .any(|member| member.credential == credential);
    //         if !is_already_member {
    //             panic!("Only existing members can rejoin")
    //         }
    //         let credential: Certificate = credential.deserialized().unwrap();
    //         let is_user = credential.is_ca();
    //         if !is_user {
    //             panic!("Only users can rejoin")
    //         }
    //         println!("{:#?}", credential);

    //         group1
    //             .merge_staged_commit(client1.provider(), *staged_commit)
    //             .unwrap();
    //     }
    //     _ => panic!("message type is controlled"),
    // }

    // // device 2 accept external join
    // let message_in = MlsMessageIn::tls_deserialize_exact_bytes(&serialized)
    //     .unwrap()
    //     .into_protocol_message()
    //     .unwrap();

    // let processed = group2
    //     .process_message(client2.provider(), message_in)
    //     .unwrap();
    // let credential = processed.credential().clone();
    // match processed.into_content() {
    //     openmls::prelude::ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
    //         let is_already_member = group2
    //             .members()
    //             .any(|member| member.credential == credential);
    //         if !is_already_member {
    //             panic!("Only existing members can rejoin")
    //         }
    //         let credential: Certificate = credential.deserialized().unwrap();
    //         let is_user = credential.is_ca();
    //         if !is_user {
    //             panic!("Only users can rejoin")
    //         }
    //         println!("{:#?}", credential);

    //         group2
    //             .merge_staged_commit(client2.provider(), *staged_commit)
    //             .unwrap();
    //     }
    //     _ => panic!("message type is controlled"),
    // }

    // let old_message = MlsMessageIn::tls_deserialize_exact_bytes(&message1)
    //     .unwrap()
    //     .into_protocol_message()
    //     .unwrap();
}

#[test]
fn test_quick_update() {
    // Some tests here have shown that for commits which add or remove members, the MLS ratchet cannot skip steps.
    // Skipping step will lead to an epoch mismatch.
    // This behaviour could also be interesting for normal application messages.
    // Maybe I can force a normal message to update the epoch too?
    // Otherwise regular key refreshs might do the same.

    // let client1 = MlsClient::new();
    // let client2 = MlsClient::new();
    // let client3 = MlsClient::new();
    // let client4 = MlsClient::new();

    // let mut group1 = client1.create_group();

    // let (_message, welcome, _state) = group1
    //     .add_members(
    //         client1.provider(),
    //         client1.credential(),
    //         &[client2.create_key_package().key_package().clone()],
    //     )
    //     .unwrap();
    // group1.merge_pending_commit(client1.provider()).unwrap();

    // let welcome =
    //     MlsMessageIn::tls_deserialize_exact_bytes(&welcome.tls_serialize_detached().unwrap())
    //         .unwrap()
    //         .into_welcome()
    //         .unwrap();

    // let mut group2 = client2.join_group(welcome);

    // let (message, _welcome, _state) = group1
    //     .add_members(
    //         client1.provider(),
    //         client1.credential(),
    //         &[client3.create_key_package().key_package().clone()],
    //     )
    //     .unwrap();
    // group1.merge_pending_commit(client1.provider()).unwrap();

    // let message =
    //     MlsMessageIn::tls_deserialize_exact_bytes(&message.tls_serialize_detached().unwrap())
    //         .unwrap()
    //         .into_protocol_message()
    //         .unwrap();
    // match group2
    //     .process_message(client2.provider(), message)
    //     .unwrap()
    //     .into_content()
    // {
    //     openmls::prelude::ProcessedMessageContent::StagedCommitMessage(commit) => group2
    //         .merge_staged_commit(client2.provider(), *commit)
    //         .unwrap(),
    //     _ => panic!("Unexpected message content"),
    // }

    // let (message, _welcome, _state) = group1
    //     .add_members(
    //         client1.provider(),
    //         client1.credential(),
    //         &[client4.create_key_package().key_package().clone()],
    //     )
    //     .unwrap();
    // group1.merge_pending_commit(client1.provider()).unwrap();

    // let message =
    //     MlsMessageIn::tls_deserialize_exact_bytes(&message.tls_serialize_detached().unwrap())
    //         .unwrap()
    //         .into_protocol_message()
    //         .unwrap();
    // match group2
    //     .process_message(client2.provider(), message)
    //     .unwrap()
    //     .into_content()
    // {
    //     openmls::prelude::ProcessedMessageContent::StagedCommitMessage(commit) => group2
    //         .merge_staged_commit(client2.provider(), *commit)
    //         .unwrap(),
    //     _ => panic!("Unexpected message content"),
    // }
}

#[test]
fn test_skipped_messages() {
    // let client1 = MlsClient::new();
    // let client2 = MlsClient::new();

    // let mut group1 = client1.create_group();
    // let (_message, welcome, _state) = group1
    //     .add_members(
    //         client1.provider(),
    //         client1.credential(),
    //         &[client2.create_key_package().key_package().clone()],
    //     )
    //     .unwrap();
    // group1.merge_pending_commit(client1.provider()).unwrap();

    // let welcome =
    //     MlsMessageIn::tls_deserialize_exact_bytes(&welcome.tls_serialize_detached().unwrap())
    //         .unwrap()
    //         .into_welcome()
    //         .unwrap();

    // let mut group2 = client2.join_group(welcome);

    // let message1 = group1
    //     .create_message(client1.provider(), client1.credential(), b"Test 1")
    //     .unwrap()
    //     .tls_serialize_detached()
    //     .unwrap();

    // let message2 = group1
    //     .create_message(client1.provider(), client1.credential(), b"Test 2")
    //     .unwrap()
    //     .tls_serialize_detached()
    //     .unwrap();

    // let received_mls_message = MlsMessageIn::tls_deserialize_exact(&mut message1.as_slice())
    //     .unwrap()
    //     .try_into_protocol_message()
    //     .unwrap();

    // match group2
    //     .process_message(client2.provider(), received_mls_message)
    //     .unwrap()
    //     .into_content()
    // {
    //     openmls::prelude::ProcessedMessageContent::ApplicationMessage(message) => {
    //         assert_eq!(message.into_bytes(), b"Test 1")
    //     }
    //     _ => panic!("message type is controlled"),
    // }

    // let received_mls_message = MlsMessageIn::tls_deserialize_exact(&mut message2.as_slice())
    //     .unwrap()
    //     .try_into_protocol_message()
    //     .unwrap();

    // match group2
    //     .process_message(client2.provider(), received_mls_message)
    //     .unwrap()
    //     .into_content()
    // {
    //     openmls::prelude::ProcessedMessageContent::ApplicationMessage(message) => {
    //         assert_eq!(message.into_bytes(), b"Test 2")
    //     }
    //     _ => panic!("message type is controlled"),
    // }

    // let message3 = group1
    //     .create_message(client1.provider(), client1.credential(), b"Test 3")
    //     .unwrap()
    //     .tls_serialize_detached()
    //     .unwrap();

    // let received_mls_message = MlsMessageIn::tls_deserialize_exact(&mut message3.as_slice())
    //     .unwrap()
    //     .try_into_protocol_message()
    //     .unwrap();

    // match group2
    //     .process_message(client2.provider(), received_mls_message)
    //     .unwrap()
    //     .into_content()
    // {
    //     openmls::prelude::ProcessedMessageContent::ApplicationMessage(message) => {
    //         assert_eq!(message.into_bytes(), b"Test 3")
    //     }
    //     _ => panic!("message type is controlled"),
    // }

    // let message4 = group1
    //     .create_message(client1.provider(), client1.credential(), b"Test 4")
    //     .unwrap()
    //     .tls_serialize_detached()
    //     .unwrap();

    // let received_mls_message = MlsMessageIn::tls_deserialize_exact(&mut message4.as_slice())
    //     .unwrap()
    //     .try_into_protocol_message()
    //     .unwrap();

    // match group2
    //     .process_message(client2.provider(), received_mls_message)
    //     .unwrap()
    //     .into_content()
    // {
    //     openmls::prelude::ProcessedMessageContent::ApplicationMessage(message) => {
    //         assert_eq!(message.into_bytes(), b"Test 4")
    //     }
    //     _ => panic!("message type is controlled"),
    // }
}

#[test]
fn custom_proposal() {
    // let client1 = MlsClient::new();
    // let client2 = MlsClient::new();

    // let mut group1 = client1.create_group();
    // let (_message, welcome, _state) = group1
    //     .add_members(
    //         client1.provider(),
    //         client1.credential(),
    //         &[client2.create_key_package().key_package().clone()],
    //     )
    //     .unwrap();
    // group1.merge_pending_commit(client1.provider()).unwrap();

    // let welcome =
    //     MlsMessageIn::tls_deserialize_exact_bytes(&welcome.tls_serialize_detached().unwrap())
    //         .unwrap()
    //         .into_welcome()
    //         .unwrap();

    // let mut group2 = client2.join_group(welcome);

    // let test_payload = vec![1, 2, 3, 4, 5, 6];
    // let proposal = CustomProposal::new(0xffff, test_payload.clone());
    // let message = group1
    //     .propose_custom_proposal_by_value(client1.provider(), client1.credential(), proposal)
    //     .unwrap()
    //     .0
    //     .tls_serialize_detached()
    //     .unwrap();

    // let received_mls_message = MlsMessageIn::tls_deserialize_exact(message.as_slice())
    //     .unwrap()
    //     .try_into_protocol_message()
    //     .unwrap();

    // match group2
    //     .process_message(client2.provider(), received_mls_message)
    //     .unwrap()
    //     .into_content()
    // {
    //     openmls::prelude::ProcessedMessageContent::ProposalMessage(proposal) => {
    //         let Proposal::Custom(custom) = proposal.proposal() else {
    //             panic!("Unexpected proposal type")
    //         };
    //         assert_eq!(custom.payload(), test_payload);
    //         group2
    //             .store_pending_proposal(client2.provider().storage(), *proposal)
    //             .unwrap();
    //     }
    //     _ => panic!("message type is controlled"),
    // }

    // let message2 = group1
    //     .commit_to_pending_proposals(client1.provider(), client1.credential())
    //     .unwrap()
    //     .0
    //     .tls_serialize_detached()
    //     .unwrap();

    // let received_mls_message = MlsMessageIn::tls_deserialize_exact(message2.as_slice())
    //     .unwrap()
    //     .try_into_protocol_message()
    //     .unwrap();

    // match group2
    //     .process_message(client2.provider(), received_mls_message)
    //     .unwrap()
    //     .into_content()
    // {
    //     openmls::prelude::ProcessedMessageContent::StagedCommitMessage(commit) => {
    //         let proposal = commit.queued_proposals().next().unwrap();
    //         let Proposal::Custom(custom) = proposal.proposal() else {
    //             panic!("Unexpected proposal type")
    //         };
    //         assert_eq!(custom.payload(), test_payload);

    //         group2
    //             .merge_staged_commit(client2.provider(), *commit)
    //             .unwrap();
    //     }
    //     _ => panic!("message type is controlled"),
    // }
}
