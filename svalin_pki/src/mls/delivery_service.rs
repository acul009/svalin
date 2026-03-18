use std::{collections::HashMap, sync::Arc};

use openmls::{
    framing::errors::ProtocolMessageError,
    group::{
        GroupId, MergeCommitError, MlsGroupJoinConfig, PURE_PLAINTEXT_WIRE_FORMAT_POLICY,
        ProcessedWelcome, ProposalStore, PublicGroup, PublicProcessMessageError, WelcomeError,
    },
    prelude::{
        CreationFromExternalError, MlsMessageBodyIn, MlsMessageIn, OpenMlsCrypto, ProtocolMessage,
        Sender, SenderRatchetConfiguration, Welcome, group_info::VerifiableGroupInfo,
    },
};
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;
use tls_codec::DeserializeBytes;
use tokio::sync::{mpsc, oneshot};

use crate::{
    CertificateParseError, SpkiHash, UnverifiedCertificate,
    mls::{
        processor::GroupCreationUnpackError,
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
            provider: SvalinProvider::new(storage_provider),
            group_cache: HashMap::new(),
        };

        tokio::task::spawn_blocking(move || {
            let mut delivery_service = delivery_service;
            while let Some(recv) = recv.blocking_recv() {
                match recv {
                    // DeliveryServiceRequest::NewGroup {
                    //     group_info,
                    //     response,
                    // } => {
                    //     let result = delivery_service.new_group(group_info);
                    //     let _ = response.send(result);
                    // }
                    DeliveryServiceRequest::ProcessMessage { message, response } => {
                        let result = delivery_service.process_message(message);
                        let _ = response.send(result);
                    }
                }
            }
        });

        Self { channel: send }
    }

    pub async fn process_message(
        &self,
        message: MlsMessageIn,
    ) -> Result<Vec<SpkiHash>, ProcessMessageError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(DeliveryServiceRequest::ProcessMessage {
                message,
                response: send,
            })
            .await;

        recv.await?
    }
}

enum DeliveryServiceRequest {
    ProcessMessage {
        message: MlsMessageIn,
        response: oneshot::Sender<Result<Vec<SpkiHash>, ProcessMessageError>>,
    },
}

struct DeliveryService {
    provider: SvalinProvider,
    group_cache: HashMap<GroupId, PublicGroup>, // device_groups: Mutex<HashMap<SpkiHash, Arc<Mutex<PublicGroup>>>>,
}

impl DeliveryService {
    // pub fn crypto(&self) -> &impl OpenMlsCrypto {
    //     self.provider.crypto()
    // }

    // fn new_group(
    //     &mut self,
    //     group_info: GroupCreationInfo,
    // ) -> Result<Vec<SpkiHash>, NewPublicGroupError> {
    //     let group_info = group_info.group_info()?;
    //     let ratchet_tree = group_info
    //         .extensions()
    //         .ratchet_tree()
    //         .ok_or_else(|| GroupCreationUnpackError::MissingRatchetTree)?
    //         .ratchet_tree()
    //         .clone();

    //     let (group, group_info) = PublicGroup::from_external(
    //         self.provider.crypto(),
    //         self.provider.storage(),
    //         ratchet_tree,
    //         group_info,
    //         ProposalStore::new(),
    //     )?;

    //     let members = group
    //         .members()
    //         .map(|member| {
    //             let cert: UnverifiedCertificate = member.credential.deserialized()?;
    //             Ok(cert.spki_hash().clone())
    //         })
    //         .collect::<Result<_, tls_codec::Error>>()?;

    //     self.group_cache
    //         .insert(group_info.group_context().group_id().clone(), group);

    //     Ok(members)
    // }

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
        message: MlsMessageIn,
    ) -> Result<Vec<SpkiHash>, ProcessMessageError> {
        let message: ProtocolMessage = match message.extract() {
            MlsMessageBodyIn::PublicMessage(public_message_in) => public_message_in.into(),
            MlsMessageBodyIn::PrivateMessage(private_message_in) => private_message_in.into(),
            MlsMessageBodyIn::Welcome(welcome) => {
                return self.process_welcome(welcome).map_err(Into::into);
            }
            MlsMessageBodyIn::GroupInfo(verifiable_group_info) => todo!(),
            MlsMessageBodyIn::KeyPackage(key_package_in) => todo!(),
        };

        let group_id = message.group_id().clone();
        let group = Self::get_group(&mut self.group_cache, self.provider.storage(), group_id)?;

        let members = group
            .members()
            .map(|member| {
                let cert: UnverifiedCertificate = member.credential.deserialized()?;
                Ok(cert.spki_hash().clone())
            })
            .collect::<Result<_, tls_codec::Error>>()?;

        let processed = group.process_message(self.provider.crypto(), message)?;

        match processed.into_content() {
            openmls::prelude::ProcessedMessageContent::ApplicationMessage(application_message) => {
                let raw = application_message.into_bytes();
                let as_str = String::from_utf8_lossy(&raw);
                println!("public application message: {}", as_str);
            }
            openmls::prelude::ProcessedMessageContent::ProposalMessage(queued_proposal) => todo!(),
            openmls::prelude::ProcessedMessageContent::ExternalJoinProposalMessage(
                queued_proposal,
            ) => todo!(),
            openmls::prelude::ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
                todo!()
            }
        }

        Ok(members)
    }
    
    fn process_group_info(&mut self, group_info: VerifiableGroupInfo) -> Result<(), ProcessGroupInfo> {
        let group_id = group_info.group_id();
        let Some(ratchet_tree) = group_info.extensions().ratchet_tree() else {
            return Err(ProcessWelcomeError::MissingRatchetTree);
        };
    }

    fn process_welcome(&mut self, welcome: Welcome) -> Result<Vec<SpkiHash>, ProcessWelcomeError> {
        let join_config = MlsGroupJoinConfig::builder()
            .max_past_epochs(0)
            .use_ratchet_tree_extension(false)
            .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .sender_ratchet_configuration(SenderRatchetConfiguration::new(0, 0))
            .build();
        let welcome = ProcessedWelcome::new_from_welcome(&self.provider, &join_config, welcome)?;

        let group_info = welcome.unverified_group_info();
        let group_id = group_info.group_id();
        let Some(ratchet_tree) = group_info.extensions().ratchet_tree() else {
            return Err(ProcessWelcomeError::MissingRatchetTree);
        };

        let group = match self.group_cache.entry(group_id.clone()) {
            // Found group in cache
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                occupied_entry.into_mut()
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                // found group in store, loading into cache
                let mls_group = if let Some(mls_group) =
                    PublicGroup::load(self.provider.storage(), vacant_entry.key())
                        .map_err(ProcessWelcomeError::StorageError)?
                {
                    mls_group
                } else {
                    // group doesn't exist yet, create it instead
                    let (public_group, _group_info) = PublicGroup::from_external(
                        self.provider.crypto(),
                        self.provider.storage(),
                        ratchet_tree.ratchet_tree().clone(),
                        group_info.clone(),
                        ProposalStore::new(),
                    )?;

                    // Todo: check if the group may even be created and if the members are as expected

                    public_group
                };

                vacant_entry.insert(mls_group)
            }
        };

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
    #[error("error decoding credential: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
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
    #[error("error processing welcome: {0}")]
    ProcessWelcomeError(#[from] ProcessWelcomeError),
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
    #[error("error deserializing certificate: {0}")]
    CredentialError(#[from] CertificateParseError),
    #[error("error merging commit: {0}")]
    MergeCommitError(
        #[from]
        MergeCommitError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
    ),
    #[error("error receiving from delivery service")]
    RecvError(#[from] oneshot::error::RecvError),
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessWelcomeError {
    #[error("error while trying to parse welcome: {0}")]
    WelcomeError(
        #[from] WelcomeError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
    ),
    #[error("the required ratchet tree expansion is not enabled")]
    MissingRatchetTree,
    #[error("storage error: {0}")]
    StorageError(<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError),
    #[error("error creating not yet existing group from welcome: {0}")]
    CreationFromExternalError(
        #[from]
        CreationFromExternalError<
            <SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError,
        >,
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
