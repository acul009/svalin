use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use openmls::{
    framing::errors::ProtocolMessageError,
    group::{
        GroupId, MergeCommitError, MlsGroup, ProposalStore, PublicGroup, PublicProcessMessageError,
    },
    prelude::{
        CreationFromExternalError, MlsMessageBodyIn, MlsMessageIn, OpenMlsCrypto, ProtocolMessage,
        PublicMessageIn, Sender,
    },
    treesync,
};
use openmls_rust_crypto::MemoryStorageError;
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;
use rustls::{lock, server::ParsedCertificate};
use tls_codec::{Deserialize, DeserializeBytes};
use tokio::sync::{mpsc, oneshot};

use crate::{
    Certificate, CertificateParseError, SpkiHash, UnverifiedCertificate,
    mls::{
        agent::EncodedReport,
        processor::{GroupCreationInfo, GroupCreationUnpackError},
        provider::{PostcardCodec, SvalinProvider},
    },
};

pub struct DeliveryServiceHandle {
    channel: mpsc::Sender<DeliveryServiceRequest>,
}

impl DeliveryServiceHandle {
    pub fn new(storage_provider: SqliteStorageProvider<PostcardCodec>) -> Self {
        let (send, mut recv) = mpsc::channel(10);

        let delivery_service = DeliveryService {
            provider: Arc::new(SvalinProvider::new(storage_provider)),
            group_cache: HashMap::new(),
        };

        std::thread::spawn(move || {
            let mut delivery_service = delivery_service;
            while let Some(recv) = recv.blocking_recv() {
                match recv {
                    DeliveryServiceRequest::NewGroup {
                        group_info,
                        response,
                    } => {
                        let result = delivery_service.new_group(group_info);
                        let _ = response.send(result);
                    }
                }
            }
        });

        Self { channel: send }
    }

    pub async fn new_group(
        &self,
        group_info: GroupCreationInfo,
    ) -> Result<(), NewPublicGroupError> {
        let (send, recv) = oneshot::channel();

        self.channel
            .send(DeliveryServiceRequest::NewGroup {
                group_info,
                response: send,
            })
            .await;

        recv.await?
    }
}

enum DeliveryServiceRequest {
    NewGroup {
        group_info: GroupCreationInfo,
        response: oneshot::Sender<Result<(), NewPublicGroupError>>,
    },
}

struct DeliveryService {
    provider: Arc<SvalinProvider>,
    group_cache: HashMap<GroupId, PublicGroup>, // device_groups: Mutex<HashMap<SpkiHash, Arc<Mutex<PublicGroup>>>>,
}

impl DeliveryService {
    pub fn crypto(&self) -> &impl OpenMlsCrypto {
        self.provider.crypto()
    }

    fn new_group(&mut self, group_info: GroupCreationInfo) -> Result<(), NewPublicGroupError> {
        let group_info = group_info.group_info()?;
        let ratchet_tree = group_info
            .extensions()
            .ratchet_tree()
            .ok_or_else(|| GroupCreationUnpackError::MissingRatchetTree)?
            .ratchet_tree()
            .clone();

        let (group, group_info) = PublicGroup::from_external(
            self.provider.crypto(),
            self.provider.storage(),
            ratchet_tree,
            group_info,
            ProposalStore::new(),
        )?;

        self.group_cache
            .insert(group_info.group_context().group_id().clone(), group);

        Ok(())
    }

    // pub async fn new_device_group(
    //     &self,
    //     group_info: DeviceGroupCreationInfo,
    // ) -> Result<(), NewPublicDeviceGroupError> {
    //     let spki_hash = group_info.certificate().spki_hash().clone();
    //     let existing = PublicGroup::load(
    //         self.provider.storage(),
    //         &GroupId::from_slice(spki_hash.as_slice()),
    //     )
    //     .map_err(NewPublicDeviceGroupError::StorageError)?;

    //     if existing.is_some() {
    //         return Err(NewPublicDeviceGroupError::GroupAlreadyExists);
    //     }

    //     let group_info = group_info.group_info()?;
    //     let ratchet_tree = group_info
    //         .extensions()
    //         .ratchet_tree()
    //         .ok_or_else(|| GroupCreationUnpackError::MissingRatchetTree)?
    //         .ratchet_tree()
    //         .clone();

    //     let (group, _group_info) = PublicGroup::from_external(
    //         self.provider.crypto(),
    //         self.provider.storage(),
    //         ratchet_tree,
    //         group_info,
    //         ProposalStore::new(),
    //     )?;

    //     let mut guard = self.device_groups.lock().unwrap();
    //     guard.insert(spki_hash, Arc::new(Mutex::new(group)));

    //     Ok(())
    // }

    fn get_group<'a>(
        cache: &'a mut HashMap<GroupId, PublicGroup>,
        storage: &<SvalinProvider as OpenMlsProvider>::StorageProvider,
        group_id: GroupId,
    ) -> Result<&'a mut PublicGroup, GetPublicGroupError> {
        let group = match cache.entry(group_id) {
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                occupied_entry.into_mut()
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                let Some(mls_group) = PublicGroup::load(storage, vacant_entry.key())
                    .map_err(GetPublicGroupError::StorageError)?
                else {
                    return Err(GetPublicGroupError::UnknownGroup);
                };

                vacant_entry.insert(mls_group)
            }
        };

        Ok(group)
    }

    fn process_message(
        &mut self,
        group_id: &[u8],
        message: &[u8],
    ) -> Result<Vec<SpkiHash>, ProcessMessageError> {
        let group_id = GroupId::from_slice(&group_id);
        let group = Self::get_group(&mut self.group_cache, self.provider.storage(), group_id)?;

        let message = MlsMessageIn::tls_deserialize_exact_bytes(message)?;

        let processed =
            group.process_message(self.provider.crypto(), message.try_into_protocol_message()?)?;

        match processed.into_content() {
            openmls::prelude::ProcessedMessageContent::ApplicationMessage(application_message) => {
                let raw = application_message.into_bytes()
                println!("public application message: {}")
                todo!()
            }
            openmls::prelude::ProcessedMessageContent::ProposalMessage(queued_proposal) => todo!(),
            openmls::prelude::ProcessedMessageContent::ExternalJoinProposalMessage(
                queued_proposal,
            ) => todo!(),
            openmls::prelude::ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
                todo!()
            }
        }

        todo!()
    }

    // pub async fn process_device_group_message(
    //     &self,
    //     device: &SpkiHash,
    //     message: &[u8],
    // ) -> Result<Vec<SpkiHash>, ProcessMessageError> {
    //     let mut guard = self.device_groups.lock().unwrap();
    //     let group = guard.get(device);
    //     let group = if let Some(group) = group {
    //         group.clone()
    //     } else {
    //         let group = PublicGroup::load(
    //             self.provider.storage(),
    //             &GroupId::from_slice(device.as_slice()),
    //         )
    //         .map_err(ProcessMessageError::StorageError)?;
    //         let Some(group) = group else {
    //             return Err(ProcessMessageError::DeviceGroupUnknown);
    //         };

    //         let group = Arc::new(Mutex::new(group));

    //         guard.insert(device.clone(), group.clone());

    //         group
    //     };
    //     drop(guard);

    //     let message = MlsMessageIn::tls_deserialize_exact_bytes(message)?.extract();
    //     let mut guard = group.lock().unwrap();

    //     if let MlsMessageBodyIn::PublicMessage(message) = message {
    //         let message = guard.process_message(self.provider.crypto(), message)?;

    //         // check if sender may even send this message;
    //         let message_sender_check = match message.content() {
    //             openmls::prelude::ProcessedMessageContent::ApplicationMessage(message) => {
    //                 todo!("check what to do with this one");
    //             }
    //             openmls::prelude::ProcessedMessageContent::ProposalMessage(queued_proposal) => {
    //                 if let Sender::Member(member) = queued_proposal.sender() {
    //                     if let Some(leaf) = guard.leaf(*member) {
    //                         let certificate = UnverifiedCertificate::from_der(
    //                             leaf.credential().serialized_content().to_vec(),
    //                         )?;
    //                         if certificate.spki_hash() == device {
    //                             todo!("check if device may send this message")
    //                         } else {
    //                             todo!("check if member may send this message")
    //                         }
    //                     } else {
    //                         Err(ProcessMessageError::MemberMessageByNonMember)
    //                     }
    //                 } else {
    //                     Err(ProcessMessageError::MessageTypeNotAllowed)
    //                 }
    //             }
    //             openmls::prelude::ProcessedMessageContent::ExternalJoinProposalMessage(
    //                 queued_proposal,
    //             ) => Err(ProcessMessageError::MessageTypeNotAllowed),
    //             openmls::prelude::ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
    //                 todo!("check when I need this one")
    //             }
    //         };

    //         // return error of message was not allowed
    //         message_sender_check?;

    //         match message.into_content() {
    //             openmls::prelude::ProcessedMessageContent::ApplicationMessage(
    //                 application_message,
    //             ) => {
    //                 // Nothing more to do, only clients can read messages, not the DS
    //             }
    //             openmls::prelude::ProcessedMessageContent::ProposalMessage(queued_proposal) => {
    //                 guard
    //                     .add_proposal(self.provider.storage(), *queued_proposal)
    //                     .map_err(ProcessMessageError::StorageError)?;
    //             }
    //             openmls::prelude::ProcessedMessageContent::ExternalJoinProposalMessage(
    //                 queued_proposal,
    //             ) => unreachable!(),
    //             openmls::prelude::ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
    //                 guard
    //                     .merge_commit(self.provider.storage(), *staged_commit)
    //                     .map_err(ProcessMessageError::MergeCommitError)?;
    //             }
    //         }
    //     }

    //     let members = guard
    //         .members()
    //         .map(|member| {
    //             let spki_hash = UnverifiedCertificate::from_der(
    //                 member.credential.serialized_content().to_vec(),
    //             )?
    //             .spki_hash()
    //             .clone();

    //             Ok(spki_hash)
    //         })
    //         .collect::<Result<Vec<_>, CertificateParseError>>()?;

    //     Ok(members)
    // }
}

#[derive(Debug, thiserror::Error)]
pub enum NewPublicGroupError {
    #[error("error unpacking group creation info: {0}")]
    GroupCreationUnpackError(#[from] GroupCreationUnpackError),
    #[error("error creating group from external: {0}")]
    CreationFromExternalError(
        #[from]
        CreationFromExternalError<
            <SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError,
        >,
    ),
    #[error("error accessing MLS storage: {0}")]
    StorageError(<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError),
    #[error("error receiving from delivery service")]
    RecvError(#[from] oneshot::error::RecvError),
}

#[derive(Debug, thiserror::Error)]
pub enum GetPublicGroupError {
    #[error("give group does not exist")]
    UnknownGroup,
    #[error("storage error: {0}")]
    StorageError(<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessMessageError {
    #[error("error getting group: {0}")]
    GetPublicGroupError(#[from] GetPublicGroupError),
    #[error("error accessing MLS storage: {0}")]
    StorageError(#[source] <SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError),
    #[error("inner error: {0}")]
    Inner(#[from] PublicProcessMessageError),
    #[error("protocol message error: {0}")]
    ProtocolMessageError(#[from] ProtocolMessageError),
    #[error("tls codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("security violation: this message type is not allowed, possible cyber attack")]
    MessageTypeNotAllowed,
    #[error("a member message was sent by a non member, probably a bug")]
    MemberMessageByNonMember,
    #[error("device group is not known by storage")]
    DeviceGroupUnknown,
    #[error("error deserializing certificate: {0}")]
    CredentialError(#[from] CertificateParseError),
    #[error("error merging commit: {0}")]
    MergeCommitError(
        #[from]
        MergeCommitError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
    ),
}

#[derive(Debug, thiserror::Error)]
pub enum NewPublicDeviceGroupError {
    #[error("error unpacking group creation info: {0}")]
    GroupCreationUnpackError(#[from] GroupCreationUnpackError),
    #[error("error creating group from external: {0}")]
    CreationFromExternalError(
        #[from]
        CreationFromExternalError<
            <SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError,
        >,
    ),
    #[error("error accessing MLS storage: {0}")]
    StorageError(<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError),
    #[error("group already exists")]
    GroupAlreadyExists,
    #[error("join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}
