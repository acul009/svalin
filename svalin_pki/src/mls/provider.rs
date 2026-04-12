use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use openmls::prelude::{Ciphersuite, ProtocolVersion};
use openmls_rust_crypto::{MemoryStorage, RustCrypto};
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;
use serde::{Deserialize, Serialize};

use crate::{DecryptError, EncryptError, EncryptedObject};

#[derive(Default)]
pub struct PostcardCodec {}

impl openmls_sqlx_storage::Codec for PostcardCodec {
    type Error = postcard::Error;

    fn to_vec<T: serde::Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, Self::Error> {
        postcard::to_stdvec(value)
    }

    fn from_slice<T: serde::de::DeserializeOwned + ?Sized>(slice: &[u8]) -> Result<T, Self::Error> {
        postcard::from_bytes(slice)
    }
}

pub struct SvalinProvider {
    crypto: RustCrypto,
    storage_provider: SvalinStorage,
    protocol_version: ProtocolVersion,
}

impl SvalinProvider {
    pub fn new(storage_provider: impl Into<SvalinStorage>) -> Self {
        let crypto = RustCrypto::default();
        Self {
            crypto,
            storage_provider: storage_provider.into(),
            protocol_version: ProtocolVersion::Mls10,
        }
    }

    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    pub fn ciphersuite(&self) -> Ciphersuite {
        Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519
    }
}

impl OpenMlsProvider for SvalinProvider {
    type CryptoProvider = RustCrypto;

    type RandProvider = RustCrypto;

    type StorageProvider = SvalinStorage;

    fn storage(&self) -> &Self::StorageProvider {
        &self.storage_provider
    }

    fn crypto(&self) -> &Self::CryptoProvider {
        &self.crypto
    }

    fn rand(&self) -> &Self::RandProvider {
        &self.crypto
    }
}

pub enum SvalinStorage {
    Sqlite(SqliteStorageProvider<PostcardCodec>),
    Memory(Arc<MemoryStorage>),
}

#[derive(Serialize, Deserialize)]
pub struct ExportedMlsStore {
    data: EncryptedObject<HashMap<Vec<u8>, Vec<u8>>>,
}

impl SvalinStorage {
    pub fn new_memory() -> (Self, ExportHandle) {
        let memory = Arc::new(MemoryStorage {
            values: RwLock::new(HashMap::new()),
        });

        (Self::Memory(memory.clone()), ExportHandle { memory })
    }

    pub async fn import(
        map: ExportedMlsStore,
        password: Vec<u8>,
    ) -> Result<(Self, ExportHandle), DecryptError> {
        let decrypted = map.data.decrypt_with_password(password).await?;
        let memory = Arc::new(MemoryStorage {
            values: RwLock::new(decrypted),
        });

        Ok((Self::Memory(memory.clone()), ExportHandle { memory }))
    }
}

pub struct ExportHandle {
    memory: Arc<MemoryStorage>,
}

impl ExportHandle {
    pub async fn export(&self, password: Vec<u8>) -> Result<ExportedMlsStore, EncryptError> {
        let store_data = self.memory.values.read().unwrap().clone();
        let encrypted = EncryptedObject::encrypt_with_password(&store_data, password).await?;

        Ok(ExportedMlsStore { data: encrypted })
    }
}

impl From<SqliteStorageProvider<PostcardCodec>> for SvalinStorage {
    fn from(sqlite: SqliteStorageProvider<PostcardCodec>) -> Self {
        Self::Sqlite(sqlite)
    }
}

impl From<MemoryStorage> for SvalinStorage {
    fn from(memory: MemoryStorage) -> Self {
        Self::Memory(Arc::new(memory))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SvalinStorageError {
    #[error(transparent)]
    SqliteError(#[from] openmls_sqlx_storage::Error),
    #[error(transparent)]
    MemoryError(#[from] openmls_rust_crypto::MemoryStorageError),
}

impl openmls_traits::storage::StorageProvider<{ openmls_traits::storage::CURRENT_VERSION }>
    for SvalinStorage
{
    type Error = SvalinStorageError;

    fn write_mls_join_config<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        MlsGroupJoinConfig: openmls_traits::storage::traits::MlsGroupJoinConfig<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
        config: &MlsGroupJoinConfig,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_mls_join_config(group_id, config)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_mls_join_config(group_id, config)
                .map_err(Into::into),
        }
    }

    fn append_own_leaf_node<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        LeafNode: openmls_traits::storage::traits::LeafNode<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
        leaf_node: &LeafNode,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .append_own_leaf_node(group_id, leaf_node)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .append_own_leaf_node(group_id, leaf_node)
                .map_err(Into::into),
        }
    }

    fn queue_proposal<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        ProposalRef: openmls_traits::storage::traits::ProposalRef<{ openmls_traits::storage::CURRENT_VERSION }>,
        QueuedProposal: openmls_traits::storage::traits::QueuedProposal<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
        proposal_ref: &ProposalRef,
        proposal: &QueuedProposal,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .queue_proposal(group_id, proposal_ref, proposal)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .queue_proposal(group_id, proposal_ref, proposal)
                .map_err(Into::into),
        }
    }

    fn write_tree<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        TreeSync: openmls_traits::storage::traits::TreeSync<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
        tree: &TreeSync,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.write_tree(group_id, tree).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.write_tree(group_id, tree).map_err(Into::into),
        }
    }

    fn write_interim_transcript_hash<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        InterimTranscriptHash: openmls_traits::storage::traits::InterimTranscriptHash<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
        interim_transcript_hash: &InterimTranscriptHash,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_interim_transcript_hash(group_id, interim_transcript_hash)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_interim_transcript_hash(group_id, interim_transcript_hash)
                .map_err(Into::into),
        }
    }

    fn write_context<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        GroupContext: openmls_traits::storage::traits::GroupContext<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
        group_context: &GroupContext,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_context(group_id, group_context)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_context(group_id, group_context)
                .map_err(Into::into),
        }
    }

    fn write_confirmation_tag<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        ConfirmationTag: openmls_traits::storage::traits::ConfirmationTag<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
        confirmation_tag: &ConfirmationTag,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_confirmation_tag(group_id, confirmation_tag)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_confirmation_tag(group_id, confirmation_tag)
                .map_err(Into::into),
        }
    }

    fn write_group_state<
        GroupState: openmls_traits::storage::traits::GroupState<{ openmls_traits::storage::CURRENT_VERSION }>,
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
        group_state: &GroupState,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_group_state(group_id, group_state)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_group_state(group_id, group_state)
                .map_err(Into::into),
        }
    }

    fn write_message_secrets<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        MessageSecrets: openmls_traits::storage::traits::MessageSecrets<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
        message_secrets: &MessageSecrets,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_message_secrets(group_id, message_secrets)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_message_secrets(group_id, message_secrets)
                .map_err(Into::into),
        }
    }

    fn write_resumption_psk_store<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        ResumptionPskStore: openmls_traits::storage::traits::ResumptionPskStore<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
        resumption_psk_store: &ResumptionPskStore,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_resumption_psk_store(group_id, resumption_psk_store)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_resumption_psk_store(group_id, resumption_psk_store)
                .map_err(Into::into),
        }
    }

    fn write_own_leaf_index<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        LeafNodeIndex: openmls_traits::storage::traits::LeafNodeIndex<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
        own_leaf_index: &LeafNodeIndex,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_own_leaf_index(group_id, own_leaf_index)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_own_leaf_index(group_id, own_leaf_index)
                .map_err(Into::into),
        }
    }

    fn write_group_epoch_secrets<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        GroupEpochSecrets: openmls_traits::storage::traits::GroupEpochSecrets<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
        group_epoch_secrets: &GroupEpochSecrets,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_group_epoch_secrets(group_id, group_epoch_secrets)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_group_epoch_secrets(group_id, group_epoch_secrets)
                .map_err(Into::into),
        }
    }

    fn write_signature_key_pair<
        SignaturePublicKey: openmls_traits::storage::traits::SignaturePublicKey<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
        SignatureKeyPair: openmls_traits::storage::traits::SignatureKeyPair<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        public_key: &SignaturePublicKey,
        signature_key_pair: &SignatureKeyPair,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_signature_key_pair(public_key, signature_key_pair)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_signature_key_pair(public_key, signature_key_pair)
                .map_err(Into::into),
        }
    }

    fn write_encryption_key_pair<
        EncryptionKey: openmls_traits::storage::traits::EncryptionKey<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
        HpkeKeyPair: openmls_traits::storage::traits::HpkeKeyPair<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        public_key: &EncryptionKey,
        key_pair: &HpkeKeyPair,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_encryption_key_pair(public_key, key_pair)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_encryption_key_pair(public_key, key_pair)
                .map_err(Into::into),
        }
    }

    fn write_encryption_epoch_key_pairs<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        EpochKey: openmls_traits::storage::traits::EpochKey<{ openmls_traits::storage::CURRENT_VERSION }>,
        HpkeKeyPair: openmls_traits::storage::traits::HpkeKeyPair<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
        epoch: &EpochKey,
        leaf_index: u32,
        key_pairs: &[HpkeKeyPair],
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_encryption_epoch_key_pairs(group_id, epoch, leaf_index, key_pairs)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_encryption_epoch_key_pairs(group_id, epoch, leaf_index, key_pairs)
                .map_err(Into::into),
        }
    }

    fn write_key_package<
        HashReference: openmls_traits::storage::traits::HashReference<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
        KeyPackage: openmls_traits::storage::traits::KeyPackage<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        hash_ref: &HashReference,
        key_package: &KeyPackage,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .write_key_package(hash_ref, key_package)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .write_key_package(hash_ref, key_package)
                .map_err(Into::into),
        }
    }

    fn write_psk<
        PskId: openmls_traits::storage::traits::PskId<{ openmls_traits::storage::CURRENT_VERSION }>,
        PskBundle: openmls_traits::storage::traits::PskBundle<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        psk_id: &PskId,
        psk: &PskBundle,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.write_psk(psk_id, psk).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.write_psk(psk_id, psk).map_err(Into::into),
        }
    }

    fn mls_group_join_config<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        MlsGroupJoinConfig: openmls_traits::storage::traits::MlsGroupJoinConfig<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Option<MlsGroupJoinConfig>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.mls_group_join_config(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.mls_group_join_config(group_id).map_err(Into::into)
            }
        }
    }

    fn own_leaf_nodes<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        LeafNode: openmls_traits::storage::traits::LeafNode<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Vec<LeafNode>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.own_leaf_nodes(group_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.own_leaf_nodes(group_id).map_err(Into::into),
        }
    }

    fn queued_proposal_refs<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        ProposalRef: openmls_traits::storage::traits::ProposalRef<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Vec<ProposalRef>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.queued_proposal_refs(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.queued_proposal_refs(group_id).map_err(Into::into)
            }
        }
    }

    fn queued_proposals<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        ProposalRef: openmls_traits::storage::traits::ProposalRef<{ openmls_traits::storage::CURRENT_VERSION }>,
        QueuedProposal: openmls_traits::storage::traits::QueuedProposal<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Vec<(ProposalRef, QueuedProposal)>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.queued_proposals(group_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.queued_proposals(group_id).map_err(Into::into),
        }
    }

    fn tree<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        TreeSync: openmls_traits::storage::traits::TreeSync<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Option<TreeSync>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.tree(group_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.tree(group_id).map_err(Into::into),
        }
    }

    fn group_context<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        GroupContext: openmls_traits::storage::traits::GroupContext<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Option<GroupContext>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.group_context(group_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.group_context(group_id).map_err(Into::into),
        }
    }

    fn interim_transcript_hash<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        InterimTranscriptHash: openmls_traits::storage::traits::InterimTranscriptHash<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Option<InterimTranscriptHash>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.interim_transcript_hash(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.interim_transcript_hash(group_id).map_err(Into::into)
            }
        }
    }

    fn confirmation_tag<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        ConfirmationTag: openmls_traits::storage::traits::ConfirmationTag<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Option<ConfirmationTag>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.confirmation_tag(group_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.confirmation_tag(group_id).map_err(Into::into),
        }
    }

    fn group_state<
        GroupState: openmls_traits::storage::traits::GroupState<{ openmls_traits::storage::CURRENT_VERSION }>,
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Option<GroupState>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.group_state(group_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.group_state(group_id).map_err(Into::into),
        }
    }

    fn message_secrets<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        MessageSecrets: openmls_traits::storage::traits::MessageSecrets<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Option<MessageSecrets>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.message_secrets(group_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.message_secrets(group_id).map_err(Into::into),
        }
    }

    fn resumption_psk_store<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        ResumptionPskStore: openmls_traits::storage::traits::ResumptionPskStore<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Option<ResumptionPskStore>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.resumption_psk_store(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.resumption_psk_store(group_id).map_err(Into::into)
            }
        }
    }

    fn own_leaf_index<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        LeafNodeIndex: openmls_traits::storage::traits::LeafNodeIndex<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Option<LeafNodeIndex>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.own_leaf_index(group_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.own_leaf_index(group_id).map_err(Into::into),
        }
    }

    fn group_epoch_secrets<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        GroupEpochSecrets: openmls_traits::storage::traits::GroupEpochSecrets<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<Option<GroupEpochSecrets>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.group_epoch_secrets(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.group_epoch_secrets(group_id).map_err(Into::into)
            }
        }
    }

    fn signature_key_pair<
        SignaturePublicKey: openmls_traits::storage::traits::SignaturePublicKey<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
        SignatureKeyPair: openmls_traits::storage::traits::SignatureKeyPair<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        public_key: &SignaturePublicKey,
    ) -> Result<Option<SignatureKeyPair>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.signature_key_pair(public_key).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.signature_key_pair(public_key).map_err(Into::into)
            }
        }
    }

    fn encryption_key_pair<
        HpkeKeyPair: openmls_traits::storage::traits::HpkeKeyPair<{ openmls_traits::storage::CURRENT_VERSION }>,
        EncryptionKey: openmls_traits::storage::traits::EncryptionKey<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        public_key: &EncryptionKey,
    ) -> Result<Option<HpkeKeyPair>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.encryption_key_pair(public_key).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.encryption_key_pair(public_key).map_err(Into::into)
            }
        }
    }

    fn encryption_epoch_key_pairs<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        EpochKey: openmls_traits::storage::traits::EpochKey<{ openmls_traits::storage::CURRENT_VERSION }>,
        HpkeKeyPair: openmls_traits::storage::traits::HpkeKeyPair<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
        epoch: &EpochKey,
        leaf_index: u32,
    ) -> Result<Vec<HpkeKeyPair>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .encryption_epoch_key_pairs(group_id, epoch, leaf_index)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .encryption_epoch_key_pairs(group_id, epoch, leaf_index)
                .map_err(Into::into),
        }
    }

    fn key_package<
        KeyPackageRef: openmls_traits::storage::traits::HashReference<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
        KeyPackage: openmls_traits::storage::traits::KeyPackage<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        hash_ref: &KeyPackageRef,
    ) -> Result<Option<KeyPackage>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.key_package(hash_ref).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.key_package(hash_ref).map_err(Into::into),
        }
    }

    fn psk<
        PskBundle: openmls_traits::storage::traits::PskBundle<{ openmls_traits::storage::CURRENT_VERSION }>,
        PskId: openmls_traits::storage::traits::PskId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        psk_id: &PskId,
    ) -> Result<Option<PskBundle>, Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.psk(psk_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.psk(psk_id).map_err(Into::into),
        }
    }

    fn remove_proposal<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        ProposalRef: openmls_traits::storage::traits::ProposalRef<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
        proposal_ref: &ProposalRef,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .remove_proposal(group_id, proposal_ref)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .remove_proposal(group_id, proposal_ref)
                .map_err(Into::into),
        }
    }

    fn delete_own_leaf_nodes<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.delete_own_leaf_nodes(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.delete_own_leaf_nodes(group_id).map_err(Into::into)
            }
        }
    }

    fn delete_group_config<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.delete_group_config(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.delete_group_config(group_id).map_err(Into::into)
            }
        }
    }

    fn delete_tree<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.delete_tree(group_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.delete_tree(group_id).map_err(Into::into),
        }
    }

    fn delete_confirmation_tag<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.delete_confirmation_tag(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.delete_confirmation_tag(group_id).map_err(Into::into)
            }
        }
    }

    fn delete_group_state<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.delete_group_state(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.delete_group_state(group_id).map_err(Into::into)
            }
        }
    }

    fn delete_context<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.delete_context(group_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.delete_context(group_id).map_err(Into::into),
        }
    }

    fn delete_interim_transcript_hash<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .delete_interim_transcript_hash(group_id)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .delete_interim_transcript_hash(group_id)
                .map_err(Into::into),
        }
    }

    fn delete_message_secrets<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.delete_message_secrets(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.delete_message_secrets(group_id).map_err(Into::into)
            }
        }
    }

    fn delete_all_resumption_psk_secrets<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .delete_all_resumption_psk_secrets(group_id)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .delete_all_resumption_psk_secrets(group_id)
                .map_err(Into::into),
        }
    }

    fn delete_own_leaf_index<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.delete_own_leaf_index(group_id).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.delete_own_leaf_index(group_id).map_err(Into::into)
            }
        }
    }

    fn delete_group_epoch_secrets<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .delete_group_epoch_secrets(group_id)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .delete_group_epoch_secrets(group_id)
                .map_err(Into::into),
        }
    }

    fn clear_proposal_queue<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        ProposalRef: openmls_traits::storage::traits::ProposalRef<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .clear_proposal_queue::<GroupId, ProposalRef>(group_id)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .clear_proposal_queue::<GroupId, ProposalRef>(group_id)
                .map_err(Into::into),
        }
    }

    fn delete_signature_key_pair<
        SignaturePublicKey: openmls_traits::storage::traits::SignaturePublicKey<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        public_key: &SignaturePublicKey,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .delete_signature_key_pair(public_key)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .delete_signature_key_pair(public_key)
                .map_err(Into::into),
        }
    }

    fn delete_encryption_key_pair<
        EncryptionKey: openmls_traits::storage::traits::EncryptionKey<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        public_key: &EncryptionKey,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .delete_encryption_key_pair(public_key)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .delete_encryption_key_pair(public_key)
                .map_err(Into::into),
        }
    }

    fn delete_encryption_epoch_key_pairs<
        GroupId: openmls_traits::storage::traits::GroupId<{ openmls_traits::storage::CURRENT_VERSION }>,
        EpochKey: openmls_traits::storage::traits::EpochKey<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        group_id: &GroupId,
        epoch: &EpochKey,
        leaf_index: u32,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite
                .delete_encryption_epoch_key_pairs(group_id, epoch, leaf_index)
                .map_err(Into::into),
            SvalinStorage::Memory(memory) => memory
                .delete_encryption_epoch_key_pairs(group_id, epoch, leaf_index)
                .map_err(Into::into),
        }
    }

    fn delete_key_package<
        KeyPackageRef: openmls_traits::storage::traits::HashReference<
                { openmls_traits::storage::CURRENT_VERSION },
            >,
    >(
        &self,
        hash_ref: &KeyPackageRef,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => {
                sqlite.delete_key_package(hash_ref).map_err(Into::into)
            }
            SvalinStorage::Memory(memory) => {
                memory.delete_key_package(hash_ref).map_err(Into::into)
            }
        }
    }

    fn delete_psk<
        PskKey: openmls_traits::storage::traits::PskId<{ openmls_traits::storage::CURRENT_VERSION }>,
    >(
        &self,
        psk_id: &PskKey,
    ) -> Result<(), Self::Error> {
        match self {
            SvalinStorage::Sqlite(sqlite) => sqlite.delete_psk(psk_id).map_err(Into::into),
            SvalinStorage::Memory(memory) => memory.delete_psk(psk_id).map_err(Into::into),
        }
    }
}
