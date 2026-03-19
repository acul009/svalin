use std::collections::HashMap;

use crate::mls::{
    key_package::{KeyPackage, KeyPackageError},
    provider::{PostcardCodec, SvalinProvider},
    transport_types::{MessageToMember, MessageToServerTransport},
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
        CredentialWithKey, KeyPackageNewError, MlsMessageBodyOut, ProtocolMessage,
        SenderRatchetConfiguration, Welcome,
    },
};
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;
use tls_codec::Serialize;
use tokio::sync::{mpsc, oneshot};

use crate::Credential;

#[derive(Clone)]
pub struct MlsProcessorHandle {
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
                    MlsProcessorRequest::ProcessMessageOld { message, response } => {
                        let result = client.process_message_old(message);
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
    ) -> Result<MessageToServerTransport, CreateGroupError> {
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
    ) -> Result<MessageToServerTransport, CreateGroupMessageError> {
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

    pub async fn process_message_old(
        &self,
        message: ProtocolMessage,
    ) -> Result<Vec<u8>, ProcessGroupMessageError> {
        let (send, recv) = oneshot::channel();

        let _ = self
            .channel
            .send(MlsProcessorRequest::ProcessMessageOld {
                message,
                response: send,
            })
            .await;

        recv.await?
    }

    pub async fn process_message(
        &self,
        message: MessageToMember,
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
        response: oneshot::Sender<Result<MessageToServerTransport, CreateGroupError>>,
    },
    CreateMessage {
        group_id: GroupId,
        message: Vec<u8>,
        response: oneshot::Sender<Result<MessageToServerTransport, CreateGroupMessageError>>,
    },
    StageJoin {
        welcome: Welcome,
        response: oneshot::Sender<Result<StagedWelcome, JoinGroupError>>,
    },
    JoinGroup {
        welcome: StagedWelcome,
        response: oneshot::Sender<Result<(), JoinGroupError>>,
    },
    ProcessMessageOld {
        message: ProtocolMessage,
        response: oneshot::Sender<Result<Vec<u8>, ProcessGroupMessageError>>,
    },
    ProcessMessage {
        message: MessageToMember,
        response: oneshot::Sender<Result<ProcessedMessage, ProcessMessageError>>,
    },
}

pub enum ProcessedMessage {
    Welcome(StagedWelcome),
    Data(Vec<u8>),
}

#[cfg(test)]
impl ProcessedMessage {
    pub fn welcome(self) -> Option<StagedWelcome> {
        match self {
            Self::Welcome(welcome) => Some(welcome),
            _ => None,
        }
    }

    pub fn data(self) -> Option<Vec<u8>> {
        match self {
            Self::Data(data) => Some(data),
            _ => None,
        }
    }
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
    ) -> Result<MessageToServerTransport, CreateGroupError> {
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

        Ok(MessageToServerTransport::NewGroup {
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
    ) -> Result<MessageToServerTransport, CreateGroupMessageError> {
        let group = Self::get_group(&mut self.group_cache, self.provider.storage(), group_id)?;

        let message = group.create_message(&self.provider, &self.svalin_credential, message)?;

        let MlsMessageBodyOut::PrivateMessage(message) = message.body() else {
            panic!("expected private message");
        };

        Ok(MessageToServerTransport::GroupMessage(
            message.tls_serialize_detached()?,
        ))
    }

    fn process_message_old(
        &mut self,
        message: ProtocolMessage,
    ) -> Result<Vec<u8>, ProcessGroupMessageError> {
        let group_id = message.group_id().clone();
        let group = Self::get_group(&mut self.group_cache, self.provider.storage(), group_id)?;

        let processed = group.process_message(&self.provider, message)?;

        match processed.into_content() {
            openmls::prelude::ProcessedMessageContent::ApplicationMessage(application_message) => {
                return Ok(application_message.into_bytes());
            }
            openmls::prelude::ProcessedMessageContent::ProposalMessage(_queued_proposal) => todo!(),
            openmls::prelude::ProcessedMessageContent::ExternalJoinProposalMessage(
                _queued_proposal,
            ) => todo!(),
            openmls::prelude::ProcessedMessageContent::StagedCommitMessage(_staged_commit) => {
                todo!()
            }
        }
    }

    fn process_message(
        &mut self,
        message: MessageToMember,
    ) -> Result<ProcessedMessage, ProcessMessageError> {
        let processed = match message {
            MessageToMember::Welcome(welcome) => {
                let staged = self.stage_join(welcome)?;
                ProcessedMessage::Welcome(staged)
            }
            MessageToMember::GroupMessage(message) => self.process_group_message(message.into())?,
        };

        Ok(processed)
    }

    fn process_group_message(
        &mut self,
        message: ProtocolMessage,
    ) -> Result<ProcessedMessage, ProcessGroupMessageError> {
        let group_id = message.group_id().clone();
        let group = Self::get_group(&mut self.group_cache, self.provider.storage(), group_id)?;

        let processed = group.process_message(&self.provider, message)?;

        match processed.into_content() {
            openmls::prelude::ProcessedMessageContent::ApplicationMessage(application_message) => {
                return Ok(ProcessedMessage::Data(application_message.into_bytes()));
            }
            openmls::prelude::ProcessedMessageContent::ProposalMessage(_queued_proposal) => todo!(),
            openmls::prelude::ProcessedMessageContent::ExternalJoinProposalMessage(
                _queued_proposal,
            ) => todo!(),
            openmls::prelude::ProcessedMessageContent::StagedCommitMessage(_staged_commit) => {
                todo!()
            }
        }
    }

    // pub async fn join_my_device_group(
    //     &self,
    //     group_info: DeviceGroupCreationInfo,
    // ) -> Result<(), JoinDeviceGroupError> {
    //     let provider = self.provider.clone();
    //     let me = self.svalin_credential.get_certificate().spki_hash().clone();
    //     let my_parent = self.svalin_credential.get_certificate().issuer().clone();

    //     let welcome = group_info.welcome()?;

    //     let ratchet_tree = group_info.ratchet_tree()?;

    //     let join_config = MlsGroupJoinConfig::builder()
    //         .max_past_epochs(0)
    //         .use_ratchet_tree_extension(false)
    //         .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
    //         .sender_ratchet_configuration(SenderRatchetConfiguration::new(0, 0))
    //         .build();

    //     let welcome = StagedWelcome::new_from_welcome(
    //         provider.as_ref(),
    //         &join_config,
    //         welcome,
    //         Some(ratchet_tree),
    //     )?;

    //     if welcome.group_context().group_id().as_slice() != me.as_slice() {
    //         return Err(JoinDeviceGroupError::WrongGroupId);
    //     }

    //     let creator: UnverifiedCertificate =
    //         welcome.welcome_sender()?.credential().deserialized()?;

    //     // Ensure there are only sessions and myself in the group
    //     welcome
    //         .members()
    //         .map(|member| -> Result<(), JoinDeviceGroupError> {
    //             let certificate: UnverifiedCertificate = member.credential.deserialized()?;
    //             if certificate.spki_hash() == &me {
    //                 return Ok(());
    //             }

    //             if certificate.certificate_type() != CertificateType::UserDevice {
    //                 return Err(JoinDeviceGroupError::WrongMemberType);
    //             }

    //             Ok(())
    //         })
    //         .collect::<Result<(), JoinDeviceGroupError>>()?;

    //     // TODO: check that members contains root
    //     if creator.issuer() != &my_parent {
    //         return Err(JoinDeviceGroupError::WrongGroupCreator);
    //     }

    //     let _group = welcome.into_group(provider.as_ref())?;

    //     Ok(())
    // }

    // pub(crate) fn signer(&self) -> &Credential {
    //     &self.svalin_credential
    // }
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessMessageError {
    #[error("error joining group:  {0}")]
    JoinGroupError(#[from] JoinGroupError),
    #[error("error processing group message")]
    ProcessGroupMessageError(#[from] ProcessGroupMessageError),
    #[error("error receiving from mlsclient")]
    RecvError(#[from] oneshot::error::RecvError),
}

#[derive(Debug, thiserror::Error)]
pub enum GetGroupError {
    #[error("give group does not exist")]
    UnknownGroup,
    #[error("storage error: {0}")]
    StorageError(<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError),
}

// #[derive(Serialize, Deserialize)]
// #[cfg_attr(test, derive(Clone))]
// pub struct DeviceGroupCreationInfo {
//     certificate: UnverifiedCertificate,
//     welcome: Vec<u8>,
//     group_info: Vec<u8>,
// }

// impl DeviceGroupCreationInfo {
//     pub fn welcome(&self) -> Result<Welcome, GroupCreationUnpackError> {
//         let message = MlsMessageIn::tls_deserialize_exact_bytes(&self.welcome.as_slice())?;

//         let MlsMessageBodyIn::Welcome(welcome) = message.extract() else {
//             return Err(GroupCreationUnpackError::WrongMessageType);
//         };

//         Ok(welcome)
//     }

//     pub fn group_info(&self) -> Result<VerifiableGroupInfo, GroupCreationUnpackError> {
//         let message = MlsMessageIn::tls_deserialize_exact_bytes(&self.group_info.as_slice())?;

//         let MlsMessageBodyIn::GroupInfo(group_info) = message.extract() else {
//             return Err(GroupCreationUnpackError::WrongMessageType);
//         };

//         Ok(group_info)
//     }

//     pub fn ratchet_tree(&self) -> Result<RatchetTreeIn, GroupCreationUnpackError> {
//         let group_info = self.group_info()?;
//         let ratchet_tree = group_info
//             .extensions()
//             .ratchet_tree()
//             .ok_or_else(|| GroupCreationUnpackError::MissingRatchetTree)?
//             .ratchet_tree()
//             .clone();

//         Ok(ratchet_tree)
//     }

//     pub fn certificate(&self) -> &UnverifiedCertificate {
//         &self.certificate
//     }
// }

// #[derive(Debug, thiserror::Error)]
// pub enum GroupCreationUnpackError {
//     #[error("error trying to deserialize mls message: {0}")]
//     TlsCoderError(#[from] tls_codec::Error),
//     #[error("wrong message type")]
//     WrongMessageType,
//     #[error("error trying to verify mls signature: {0}")]
//     SignatureError(#[from] openmls::prelude::SignatureError),
//     #[error("missing ratchet tree extension")]
//     MissingRatchetTree,
//     #[error("error trying to verify mls ratchet tree: {0}")]
//     RatchetTreeError(#[from] openmls::treesync::RatchetTreeError),
// }

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
pub enum ProcessGroupMessageError {
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
    #[error("error receiving from mlsprocessor")]
    RecvError(#[from] oneshot::error::RecvError),
}

// #[derive(Debug, thiserror::Error)]
// pub enum CreateDeviceGroupError {
//     #[error("error trying to create mls group: {0}")]
//     NewGroupError(
//         #[from] NewGroupError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
//     ),
//     #[error("error trying to add members to mls group: {0}")]
//     AddMembersError(
//         #[from]
//         AddMembersError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
//     ),
//     #[error("error trying to merge pending commit: {0}")]
//     MergePendingCommitError(
//         #[from]
//         MergePendingCommitError<
//             <SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError,
//         >,
//     ),
//     #[error("error trying to create mls message: {0}")]
//     MlsMessageError(#[from] MlsMessageError),
//     #[error("error in tls codec: {0}")]
//     TlsCodecError(#[from] tls_codec::Error),
//     #[error("error trying to export group info: {0}")]
//     ExportGroupInfoError(#[from] ExportGroupInfoError),
//     #[error("error trying to join task: {0}")]
//     TokioJoinError(#[from] tokio::task::JoinError),
// }

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

// #[derive(Debug, thiserror::Error)]
// pub enum JoinDeviceGroupError {
//     #[error("group creation unpack error: {0}")]
//     GroupCreationUnpackError(#[from] GroupCreationUnpackError),
//     #[error("error while trying to parse welcome: {0}")]
//     WelcomeError(
//         #[from] WelcomeError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
//     ),
//     #[error("error in openmls library: {0}")]
//     LibraryError(#[from] openmls::error::LibraryError),
//     #[error("error in tls codec: {0}")]
//     TlsCodecError(#[from] tls_codec::Error),
//     #[error("wrong group creator")]
//     WrongGroupCreator,
//     #[error("wrong group id, not my group")]
//     WrongGroupId,
//     #[error("wrong member type")]
//     WrongMemberType,
// }
