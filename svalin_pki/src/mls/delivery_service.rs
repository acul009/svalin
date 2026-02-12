use std::sync::Arc;

use openmls::{
    group::{GroupId, ProposalStore, PublicGroup},
    prelude::{CreationFromExternalError, OpenMlsCrypto},
    treesync,
};
use openmls_rust_crypto::MemoryStorageError;
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;

use crate::{
    Certificate,
    mls::{
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
}

impl DeliveryService {
    pub fn new(storage_provider: SqliteStorageProvider<PostcardCodec>) -> Self {
        Self {
            provider: Arc::new(SvalinProvider::new(storage_provider)),
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
            let existing = PublicGroup::load(
                provider.storage(),
                &GroupId::from_slice(group_info.certificate().spki_hash().as_slice()),
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

            let (_group, _group_info) = PublicGroup::from_external(
                provider.crypto(),
                provider.storage(),
                ratchet_tree,
                group_info,
                ProposalStore::new(),
            )?;

            Ok(())
        })
        .await
        .map_err(NewPublicDeviceGroupError::JoinError)
        .flatten()
    }
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
