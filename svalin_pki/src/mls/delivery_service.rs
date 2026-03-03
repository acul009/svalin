use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use openmls::{
    group::{GroupId, MergeCommitError, MlsGroup, ProposalStore, PublicGroup},
    prelude::{CreationFromExternalError, OpenMlsCrypto, Sender},
    treesync,
};
use openmls_rust_crypto::MemoryStorageError;
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;
use rustls::lock;

use crate::{
    Certificate, SpkiHash,
    mls::{
        agent::EncodedReport,
        client::{DeviceGroupCreationInfo, GroupCreationUnpackError},
        provider::{PostcardCodec, SvalinProvider},
    },
};

#[derive(Debug, thiserror::Error)]
pub enum CreateRoomError {
    #[error("Missing ratchet tree in group info")]
    MissingRatchetTree,
    #[error("Treesync Ratchet tree error: {0}")]
    RatchetTreeError(#[from] treesync::RatchetTreeError),
    #[error("Public Group creation error: {0}")]
    CreateFromExternalError(#[from] CreationFromExternalError<MemoryStorageError>),
}

#[derive(Debug, thiserror::Error)]
pub enum AddNewMemberError {}

pub struct DeliveryService {
    provider: Arc<SvalinProvider>,
    device_groups: Mutex<HashMap<SpkiHash, Arc<Mutex<PublicGroup>>>>,
}

impl DeliveryService {
    pub fn new(storage_provider: SqliteStorageProvider<PostcardCodec>) -> Self {
        Self {
            provider: Arc::new(SvalinProvider::new(storage_provider)),
            device_groups: Mutex::new(HashMap::new()),
        }
    }

    pub fn crypto(&self) -> &impl OpenMlsCrypto {
        self.provider.crypto()
    }

    pub async fn new_device_group(
        &self,
        group_info: DeviceGroupCreationInfo,
    ) -> Result<(), NewPublicDeviceGroupError> {
        let provider = self.provider.clone();
        tokio::task::spawn_blocking(move || {
            let spki_hash = group_info.certificate().spki_hash().clone();
            let existing = PublicGroup::load(
                provider.storage(),
                &GroupId::from_slice(spki_hash.as_slice()),
            )
            .map_err(NewPublicDeviceGroupError::StorageError)?;

            if existing.is_some() {
                return Err(NewPublicDeviceGroupError::GroupAlreadyExists);
            }

            let group_info = group_info.group_info()?;
            let ratchet_tree = group_info
                .extensions()
                .ratchet_tree()
                .ok_or_else(|| GroupCreationUnpackError::MissingRatchetTree)?
                .ratchet_tree()
                .clone();

            let (group, _group_info) = PublicGroup::from_external(
                provider.crypto(),
                provider.storage(),
                ratchet_tree,
                group_info,
                ProposalStore::new(),
            )?;

            let mut guard = self.device_groups.lock()?;
            guard.insert(spki_hash, Arc::new(Mutex::new(group)));

            Ok(())
        })
        .await
        .map_err(NewPublicDeviceGroupError::JoinError)
        .flatten()
    }

    pub async fn process_device_group_message(
        &self,
        device: &SpkiHash,
        message: Vec<u8>,
    ) -> Result<(), ProcessMessageError> {
        let guard = self.device_groups.lock()?;
        let group = guard.get(device);
        let group = if let Some(group) = group {
            group.clone()
        } else {
            let group = PublicGroup::load(
                self.provider.storage(),
                &GroupId::from_slice(device.as_slice()),
            )?;
            let Some(group) = group else {
                return Err(());
            };

            let group = Arc::new(Mutex::new(group));

            guard.insert(device.clone(), group.clone());

            group
        };
        drop(guard);

        let guard = group.lock()?;
        let message = guard.process_message(self.provider.crypto(), message)?;

        // check if sender may even send this message;
        let message_sender_check = match message.content() {
            openmls::prelude::ProcessedMessageContent::ApplicationMessage(application_message) => {
                if let Sender::Member(member) = queued_proposal.sender() {
                    if let Some(leaf) = guard.leaf(member) {
                        let certificate: Certificate = leaf.credential().deserialized()?;
                        if certificate.spki_hash() == device {
                            // Only the device may send reports to this group
                            Ok(())
                        } else {
                            Err(ProcessMessageError::MessageTypeNotAllowed)
                        }
                    } else {
                        Err(ProcessMessageError::MemberMessageByNonMember)
                    }
                } else {
                    Err(ProcessMessageError::MessageTypeNotAllowed)
                }
            }
            openmls::prelude::ProcessedMessageContent::ProposalMessage(queued_proposal) => {
                if let Sender::Member(member) = queued_proposal.sender() {
                    if let Some(leaf) = guard.leaf(member) {
                        let certificate: Certificate = leaf.credential().deserialized()?;
                        if certificate.spki_hash() == device {
                            todo!("check if device may send this message")
                        } else {
                            todo!("check if member may send this message")
                        }
                    } else {
                        Err(ProcessMessageError::MemberMessageByNonMember)
                    }
                } else {
                    Err(ProcessMessageError::MessageTypeNotAllowed)
                }
            }
            openmls::prelude::ProcessedMessageContent::ExternalJoinProposalMessage(
                queued_proposal,
            ) => Err(ProcessMessageError::MessageTypeNotAllowed),
            openmls::prelude::ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
                if let Sender::Member(member) = queued_proposal.sender() {
                    if let Some(leaf) = guard.leaf(member) {
                        let certificate: Certificate = leaf.credential().deserialized()?;
                        if certificate.spki_hash() == device {
                            todo!("check if device may send this message")
                        } else {
                            todo!("check if member may send this message")
                        }
                    } else {
                        Err(ProcessMessageError::MemberMessageByNonMember)
                    }
                } else {
                    Err(ProcessMessageError::MessageTypeNotAllowed)
                }
            }
        };

        // return error of message was not allowed
        message_sender_check?;

        match message.into_content() {
            openmls::prelude::ProcessedMessageContent::ApplicationMessage(application_message) => {
                // Nothing more to do, only clients can read messages, not the DS
            }
            openmls::prelude::ProcessedMessageContent::ProposalMessage(queued_proposal) => {
                guard.add_proposal(self.provider.storage(), *queued_proposal)?;
            }
            openmls::prelude::ProcessedMessageContent::ExternalJoinProposalMessage(
                queued_proposal,
            ) => unreachable!(),
            openmls::prelude::ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
                guard.merge_commit(self.provider.storage(), staged_commit)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessMessageError {
    #[error("error accessing MLS storage: {0}")]
    StorageError(#[from] <SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError),
    #[error("security violation: this message type is not allowed, possible cyber attack")]
    MessageTypeNotAllowed,
    #[error("a member message was sent by a non member, probably a bug")]
    MemberMessageByNonMember,
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
