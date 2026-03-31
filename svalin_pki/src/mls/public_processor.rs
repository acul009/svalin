use std::{
    collections::{HashMap, HashSet},
    ops::Add,
};

use openmls::{
    framing::errors::ProtocolMessageError,
    group::{GroupId, MergeCommitError, ProposalStore, PublicGroup, PublicProcessMessageError},
    prelude::{CreationFromExternalError, MlsMessageIn, group_info::VerifiableGroupInfo},
};
use openmls_rust_crypto::{MemoryStorage, MemoryStorageError};
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;
use tls_codec::DeserializeBytes;
use tokio::sync::{mpsc, oneshot};

use crate::{
    CertificateParseError, SpkiHash,
    mls::{
        provider::{PostcardCodec, SvalinProvider},
        transport_types::{MessageToMemberTransport, MessageToSend, MessageToServer, NewGroup},
    },
};

pub(crate) struct PublicProcessorHandle {
    channel: mpsc::Sender<PublicProcessorRequest>,
}

impl PublicProcessorHandle {
    pub fn new(storage_provider: SqliteStorageProvider<PostcardCodec>) -> Self {
        let (send, mut recv) = mpsc::channel(10);

        let public_processor = PublicProcessor {
            provider: SvalinProvider::new(storage_provider),
            group_cache: HashMap::new(),
        };

        tokio::task::spawn_blocking(move || {
            let mut public_processor = public_processor;
            while let Some(recv) = recv.blocking_recv() {
                match recv {
                    PublicProcessorRequest::ProcessMessage { message, response } => {
                        let result = public_processor.process_message(message);
                        let _ = response.send(result);
                    }
                    PublicProcessorRequest::CheckGroup {
                        new_group,
                        response,
                    } => {
                        let result = public_processor.check_group(new_group);
                        let _ = response.send(result);
                    }
                    PublicProcessorRequest::AddGroup {
                        new_group,
                        response,
                    } => {
                        let result = public_processor.add_group(new_group);
                        let _ = response.send(result);
                    }
                }
            }
        });

        Self { channel: send }
    }

    pub async fn process_message(
        &self,
        message: Vec<u8>,
    ) -> Result<MessageToSend, ProcessMessageError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(PublicProcessorRequest::ProcessMessage {
                message,
                response: send,
            })
            .await;

        recv.await?
    }

    pub async fn check_group(&self, new_group: NewGroup) -> Result<PublicGroup, AddGroupError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(PublicProcessorRequest::CheckGroup {
                new_group,
                response: send,
            })
            .await;

        recv.await?
    }

    pub async fn add_group(&self, new_group: NewGroup) -> Result<MessageToSend, AddGroupError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(PublicProcessorRequest::AddGroup {
                new_group,
                response: send,
            })
            .await;

        recv.await?
    }
}

enum PublicProcessorRequest {
    ProcessMessage {
        message: Vec<u8>,
        response: oneshot::Sender<Result<MessageToSend, ProcessMessageError>>,
    },
    AddGroup {
        new_group: NewGroup,
        response: oneshot::Sender<Result<MessageToSend, AddGroupError>>,
    },
    CheckGroup {
        new_group: NewGroup,
        response: oneshot::Sender<Result<PublicGroup, AddGroupError>>,
    },
}

struct PublicProcessor {
    provider: SvalinProvider,
    group_cache: HashMap<GroupId, PublicGroup>,
}

impl PublicProcessor {
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

    fn process_message(&mut self, message: Vec<u8>) -> Result<MessageToSend, ProcessMessageError> {
        let mls_message = MlsMessageIn::tls_deserialize_exact_bytes(&message)?;
        let protocol_message = mls_message.try_into_protocol_message()?;

        let members = self.get_group_members(protocol_message.group_id().clone())?;

        let to_send = MessageToSend {
            receivers: members,
            message: MessageToMemberTransport::GroupMessage(message),
        };

        Ok(to_send)
    }

    fn check_group(&mut self, new_group: NewGroup) -> Result<PublicGroup, AddGroupError> {
        let Some(ratchet_tree) = new_group.group_info.extensions().ratchet_tree() else {
            return Err(AddGroupError::MissingRatchetTree);
        };

        let temp_storage = MemoryStorage::default();

        let (group, _) = PublicGroup::from_external(
            self.provider.crypto(),
            &temp_storage,
            ratchet_tree.ratchet_tree().clone(),
            new_group.group_info,
            ProposalStore::new(),
        )
        .map_err(AddGroupError::TempCreationFromExternalError)?;

        Ok(group)
    }

    fn add_group(&mut self, new_group: NewGroup) -> Result<MessageToSend, AddGroupError> {
        let Some(ratchet_tree) = new_group.group_info.extensions().ratchet_tree() else {
            return Err(AddGroupError::MissingRatchetTree);
        };

        let (group, _) = PublicGroup::from_external(
            self.provider.crypto(),
            self.provider.storage(),
            ratchet_tree.ratchet_tree().clone(),
            new_group.group_info,
            ProposalStore::new(),
        )
        .map_err(AddGroupError::CreationFromExternalError)?;

        let members = group
            .members()
            .map(|member| {
                let spki_hash: SpkiHash = member.credential.deserialized()?;
                Ok(spki_hash)
            })
            .collect::<Result<Vec<SpkiHash>, tls_codec::Error>>()?;

        Ok(MessageToSend {
            receivers: members,
            message: MessageToMemberTransport::Welcome(new_group.welcome),
        })
    }

    fn get_group_members(&mut self, group_id: GroupId) -> Result<Vec<SpkiHash>, GetMembersError> {
        let group = Self::get_group(&mut self.group_cache, self.provider.storage(), group_id)?;

        let members = group
            .members()
            .map(|member| {
                let spki_hash: SpkiHash = member.credential.deserialized()?;
                Ok(spki_hash)
            })
            .collect::<Result<_, tls_codec::Error>>()?;

        Ok(members)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessMessageError {
    #[error("error during tls decode: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("message seems to have wrong format: {0}")]
    ProtocolMessageError(#[from] ProtocolMessageError),
    #[error("error getting members: {0}")]
    GetMembersError(#[from] GetMembersError),
    #[error("error receiving from delivery service")]
    RecvError(#[from] oneshot::error::RecvError),
}

#[derive(Debug, thiserror::Error)]
pub enum AddGroupError {
    #[error("the group info is missing the ratchet tree extension")]
    MissingRatchetTree,
    #[error("error getting group: {0}")]
    GetPublicGroupError(#[from] GetPublicGroupError),
    #[error("error creating group from external: {0}")]
    TempCreationFromExternalError(#[source] CreationFromExternalError<MemoryStorageError>),
    #[error("error creating group from external: {0}")]
    CreationFromExternalError(
        #[source]
        CreationFromExternalError<
            <SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError,
        >,
    ),
    #[error("tls codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("error receiving from delivery service")]
    RecvError(#[from] oneshot::error::RecvError),
}

#[derive(Debug, thiserror::Error)]
pub enum GetMembersError {
    #[error("error getting group: {0}")]
    GetPublicGroupError(#[from] GetPublicGroupError),
    #[error("tls codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum GetPublicGroupError {
    #[error("give group does not exist")]
    UnknownGroup,
    #[error("storage error: {0}")]
    StorageError(<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessMessageErrorOld {
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
