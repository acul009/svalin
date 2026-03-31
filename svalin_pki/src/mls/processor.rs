use std::collections::HashMap;

use crate::{
    SpkiHash,
    mls::{
        key_package::{KeyPackage, KeyPackageError},
        provider::{PostcardCodec, SvalinProvider},
        transport_types::{MessageToMember, MessageToServer, NewGroup, NewGroupTransport},
    },
};
use openmls::{
    framing::errors::{MlsMessageError, ProtocolMessageError},
    group::{
        AddMembersError, CommitBuilderStageError, CreateCommitError, CreateMessageError,
        ExportGroupInfoError, GroupId, MergePendingCommitError, MlsGroup, MlsGroupJoinConfig,
        NewGroupError, PURE_PLAINTEXT_WIRE_FORMAT_POLICY, ProcessedWelcome, StagedWelcome,
        WelcomeError,
    },
    prelude::{
        CredentialWithKey, KeyPackageNewError, MlsMessageBodyIn, MlsMessageBodyOut, MlsMessageIn,
        PrivateMessageIn, ProtocolMessage, Sender, SenderRatchetConfiguration, Welcome,
    },
};
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;
use tls_codec::{DeserializeBytes, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::Credential;

#[derive(Clone)]
pub(crate) struct MlsProcessorHandle {
    channel: mpsc::Sender<MlsProcessorRequest>,
}

impl MlsProcessorHandle {
    pub fn new_processor(
        credential: Credential,
        storage_provider: SqliteStorageProvider<PostcardCodec>,
    ) -> Self {
        let public_info = CredentialWithKey {
            credential: credential.certificate().spki_hash().into(),
            signature_key: credential.certificate().public_key().into(),
        };

        let (send, mut recv) = mpsc::channel(10);

        let client = MlsProcessor {
            provider: SvalinProvider::new(storage_provider),
            svalin_credential: credential,
            mls_credential_with_key: public_info,
            group_cache: HashMap::new(),
        };

        tokio::task::spawn_blocking(move || {
            let mut client = client;
            while let Some(request) = recv.blocking_recv() {
                match request {
                    MlsProcessorRequest::CreateKeyPackage { response } => {
                        let result = client.create_key_package();
                        let _ = response.send(result);
                    }
                    MlsProcessorRequest::CreateGroup {
                        members,
                        group_id,
                        response,
                    } => {
                        let result = client.create_group(members, group_id);
                        let _ = response.send(result);
                    }
                    MlsProcessorRequest::StageJoin { welcome, response } => {
                        let result = client.stage_join(welcome);
                        let _ = response.send(result);
                    }
                    MlsProcessorRequest::JoinGroup { welcome, response } => {
                        let result = client.join_group(welcome);
                        let _ = response.send(result);
                    }
                    MlsProcessorRequest::CreateMessage {
                        group_id,
                        message,
                        response,
                    } => {
                        let result = client.create_message(group_id, &message);
                        let _ = response.send(result);
                    }
                    MlsProcessorRequest::ProcessMessage { message, response } => {
                        let result = client.process_message(message);
                        let _ = response.send(result);
                    }
                }
            }
        });

        MlsProcessorHandle { channel: send }
    }

    pub async fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(MlsProcessorRequest::CreateKeyPackage { response: send })
            .await;

        recv.await?
    }

    pub async fn create_group(
        &self,
        members: Vec<KeyPackage>,
        group_id: GroupId,
    ) -> Result<NewGroupTransport, CreateGroupError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(MlsProcessorRequest::CreateGroup {
                members,
                group_id,
                response: send,
            })
            .await;

        recv.await?
    }

    pub async fn stage_join(&self, welcome: Welcome) -> Result<StagedWelcome, JoinGroupError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(MlsProcessorRequest::StageJoin {
                welcome,
                response: send,
            })
            .await;

        recv.await?
    }

    pub async fn join_group(&self, welcome: StagedWelcome) -> Result<(), JoinGroupError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(MlsProcessorRequest::JoinGroup {
                welcome,
                response: send,
            })
            .await;

        recv.await?
    }

    pub async fn create_message(
        &self,
        group_id: GroupId,
        message: Vec<u8>,
    ) -> Result<MessageToServer, CreateGroupMessageError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(MlsProcessorRequest::CreateMessage {
                group_id,
                message,
                response: send,
            })
            .await;

        recv.await?
    }

    pub async fn process_message(
        &self,
        message: PrivateMessageIn,
    ) -> Result<ProcessedMessage, ProcessMessageError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(MlsProcessorRequest::ProcessMessage {
                message,
                response: send,
            })
            .await;

        recv.await?
    }
}

enum MlsProcessorRequest {
    CreateKeyPackage {
        response: oneshot::Sender<Result<KeyPackage, CreateKeyPackageError>>,
    },
    CreateGroup {
        members: Vec<KeyPackage>,
        group_id: GroupId,
        response: oneshot::Sender<Result<NewGroupTransport, CreateGroupError>>,
    },
    CreateMessage {
        group_id: GroupId,
        message: Vec<u8>,
        response: oneshot::Sender<Result<MessageToServer, CreateGroupMessageError>>,
    },
    StageJoin {
        welcome: Welcome,
        response: oneshot::Sender<Result<StagedWelcome, JoinGroupError>>,
    },
    JoinGroup {
        welcome: StagedWelcome,
        response: oneshot::Sender<Result<(), JoinGroupError>>,
    },
    ProcessMessage {
        message: PrivateMessageIn,
        response: oneshot::Sender<Result<ProcessedMessage, ProcessMessageError>>,
    },
}

pub(crate) struct ProcessedMessage {
    pub group_id: GroupId,
    pub sender: SpkiHash,
    pub decrypted: Vec<u8>,
}

struct MlsProcessor {
    // Needs to be an Arc so `spawn_blocking` works.
    provider: SvalinProvider,
    svalin_credential: Credential,
    mls_credential_with_key: CredentialWithKey,
    group_cache: HashMap<GroupId, MlsGroup>,
}

impl MlsProcessor {
    fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        let mls_key_package = openmls::prelude::KeyPackage::builder()
            .build(
                self.provider.ciphersuite(),
                &self.provider,
                &self.svalin_credential,
                self.mls_credential_with_key.clone(),
            )?
            .key_package()
            .clone();

        let key_package = KeyPackage::new(
            self.svalin_credential.certificate().clone(),
            mls_key_package,
        )?;

        Ok(key_package)
    }

    fn create_group(
        &mut self,
        members: Vec<KeyPackage>,
        group_id: GroupId,
    ) -> Result<NewGroupTransport, CreateGroupError> {
        let group = MlsGroup::builder()
            .ciphersuite(self.provider.ciphersuite())
            .with_group_id(group_id.clone())
            .use_ratchet_tree_extension(true)
            .sender_ratchet_configuration(SenderRatchetConfiguration::new(0, 0))
            // Needs to be plaintext so the server can track the group members
            .with_wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .build(
                &self.provider,
                &self.svalin_credential,
                self.mls_credential_with_key.clone(),
            )?;

        let mut entry = self.group_cache.entry(group_id).insert_entry(group);
        let group = entry.get_mut();

        let mls_key_packages = members.into_iter().map(KeyPackage::unpack);

        let bundle = group
            .commit_builder()
            .propose_adds(mls_key_packages)
            .force_self_update(true)
            .load_psks(self.provider.storage())?
            .build(
                self.provider.rand(),
                self.provider.crypto(),
                &self.svalin_credential,
                |_| true,
            )?
            .stage_commit(&self.provider)?;

        // Commit message is not needed, since there are no other members yet
        let (_commit_message, welcome, group_info) = bundle.into_contents();

        let group_info = group_info.expect("ratchet tree extension is enabled");
        let welcome = welcome.expect("add proposals will return a welcome");

        group.merge_pending_commit(&self.provider)?;

        Ok(NewGroupTransport {
            group_info: group_info.tls_serialize_detached()?,
            welcome: welcome.tls_serialize_detached()?,
        })
    }

    fn stage_join(&mut self, welcome: Welcome) -> Result<StagedWelcome, JoinGroupError> {
        let join_config = MlsGroupJoinConfig::builder()
            .max_past_epochs(0)
            .use_ratchet_tree_extension(false)
            .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .sender_ratchet_configuration(SenderRatchetConfiguration::new(0, 0))
            .build();

        let processed = ProcessedWelcome::new_from_welcome(&self.provider, &join_config, welcome)?;

        let welcome = processed.into_staged_welcome(&self.provider, None)?;

        Ok(welcome)
    }

    fn join_group(&mut self, welcome: StagedWelcome) -> Result<(), JoinGroupError> {
        let group = welcome.into_group(&self.provider)?;

        self.group_cache.insert(group.group_id().clone(), group);

        Ok(())
    }

    fn get_group<'a>(
        cache: &'a mut HashMap<GroupId, MlsGroup>,
        storage: &<SvalinProvider as OpenMlsProvider>::StorageProvider,
        group_id: GroupId,
    ) -> Result<&'a mut MlsGroup, GetGroupError> {
        let group = match cache.entry(group_id) {
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                occupied_entry.into_mut()
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                let Some(mls_group) = MlsGroup::load(storage, vacant_entry.key())
                    .map_err(GetGroupError::StorageError)?
                else {
                    return Err(GetGroupError::UnknownGroup);
                };

                vacant_entry.insert(mls_group)
            }
        };

        Ok(group)
    }

    fn create_message(
        &mut self,
        group_id: GroupId,
        message: &[u8],
    ) -> Result<MessageToServer, CreateGroupMessageError> {
        let group = Self::get_group(&mut self.group_cache, self.provider.storage(), group_id)?;

        let message = group.create_message(&self.provider, &self.svalin_credential, message)?;

        let MlsMessageBodyOut::PrivateMessage(_) = message.body() else {
            panic!("expected private message");
        };

        Ok(MessageToServer::GroupMessage(
            message.tls_serialize_detached()?,
        ))
    }

    fn process_message(
        &mut self,
        message: PrivateMessageIn,
    ) -> Result<ProcessedMessage, ProcessMessageError> {
        let message: ProtocolMessage = message.into();
        let group_id = message.group_id().clone();
        let group = Self::get_group(
            &mut self.group_cache,
            self.provider.storage(),
            group_id.clone(),
        )?;

        let processed = group.process_message(&self.provider, message)?;
        let Sender::Member(sender) = processed.sender() else {
            return Err(ProcessMessageError::InvalidSender);
        };
        let sender: SpkiHash = group
            .member_at(sender.clone())
            .expect("sender index should already have been checked")
            .credential
            .deserialized()?;

        match processed.into_content() {
            openmls::prelude::ProcessedMessageContent::ApplicationMessage(application_message) => {
                Ok(ProcessedMessage {
                    group_id,
                    sender,
                    decrypted: application_message.into_bytes(),
                })
            }
            openmls::prelude::ProcessedMessageContent::ProposalMessage(_queued_proposal) => {
                Err(ProcessMessageError::ForbiddenMessageType)
            }
            openmls::prelude::ProcessedMessageContent::ExternalJoinProposalMessage(
                _queued_proposal,
            ) => Err(ProcessMessageError::ForbiddenMessageType),
            openmls::prelude::ProcessedMessageContent::StagedCommitMessage(_staged_commit) => {
                Err(ProcessMessageError::ForbiddenMessageType)
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetGroupError {
    #[error("give group does not exist")]
    UnknownGroup,
    #[error("storage error: {0}")]
    StorageError(<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum CreateKeyPackageError {
    #[error("error trying to create mls key package: {0}")]
    KeyPackageNewError(#[from] KeyPackageNewError),
    #[error("error trying to serialize mls key package: {0}")]
    SerializationError(#[from] tls_codec::Error),
    #[error("error trying to create mls key package: {0}")]
    KeyPackageError(#[from] KeyPackageError),
    #[error("error receiving from mlsclient")]
    RecvError(#[from] oneshot::error::RecvError),
}

#[derive(Debug, thiserror::Error)]
pub enum CreateGroupError {
    #[error("error trying to create mls group: {0}")]
    NewGroupError(
        #[from] NewGroupError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
    ),
    #[error("error trying to add members to mls group: {0}")]
    AddMembersError(
        #[from]
        AddMembersError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
    ),
    #[error("error adding add commit: {0}")]
    CreateCommitError(#[from] CreateCommitError),
    #[error("error staging add commit: {0}")]
    CommitBuilderStageError(
        #[from]
        CommitBuilderStageError<
            <SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError,
        >,
    ),
    #[error("error trying to merge pending commit: {0}")]
    MergePendingCommitError(
        #[from]
        MergePendingCommitError<
            <SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError,
        >,
    ),
    #[error("error trying to create mls message: {0}")]
    MlsMessageError(#[from] MlsMessageError),
    #[error("error in tls codec: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("error trying to export group info: {0}")]
    ExportGroupInfoError(#[from] ExportGroupInfoError),
    #[error("error receiving from mlsprocessor")]
    RecvError(#[from] oneshot::error::RecvError),
}

#[derive(Debug, thiserror::Error)]
pub enum CreateGroupMessageError {
    #[error("error loading group: {0}")]
    GetGroupError(#[from] GetGroupError),
    #[error("mls error: {0}")]
    Inner(#[from] CreateMessageError),
    #[error("error in tls codec: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("error receiving from mlsprocessor")]
    RecvError(#[from] oneshot::error::RecvError),
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessMessageError {
    #[error("error loading group: {0}")]
    GetGroupError(#[from] GetGroupError),
    #[error("error decoding message: {0}")]
    DecodeError(#[from] tls_codec::Error),
    #[error("protocol message error: {0}")]
    ProtocolMessageError(#[from] ProtocolMessageError),
    #[error("process message error: {0}")]
    ProcessError(
        #[from]
        openmls::group::ProcessMessageError<
            <SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError,
        >,
    ),
    #[error("group message contained forbidden message type")]
    ForbiddenMessageType,
    #[error("invalid sender")]
    InvalidSender,
    #[error("error receiving from mlsprocessor")]
    RecvError(#[from] oneshot::error::RecvError),
}

#[derive(Debug, thiserror::Error)]
pub enum JoinGroupError {
    #[error("error while trying to parse welcome: {0}")]
    WelcomeError(
        #[from] WelcomeError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
    ),
    #[error("error in openmls library: {0}")]
    LibraryError(#[from] openmls::error::LibraryError),
    #[error("error in tls codec: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("error receiving from mlsprocessor")]
    RecvError(#[from] oneshot::error::RecvError),
}
