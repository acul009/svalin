use std::sync::Arc;

use openmls::{
    group::{ProposalStore, PublicGroup},
    prelude::{
        CreationFromExternalError, MlsMessageIn, ProtocolVersion, Verifiable,
        group_info::VerifiableGroupInfo,
    },
    treesync,
};
use openmls_rust_crypto::{MemoryStorage, MemoryStorageError, RustCrypto};
use openmls_traits::OpenMlsProvider;

#[derive(Debug)]
pub struct DsProvider {
    crypto: RustCrypto,
    key_store: MemoryStorage,
    protocol_version: ProtocolVersion,
}

impl DsProvider {
    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }
}

impl OpenMlsProvider for DsProvider {
    type CryptoProvider = RustCrypto;

    type RandProvider = RustCrypto;

    type StorageProvider = MemoryStorage;

    fn storage(&self) -> &Self::StorageProvider {
        &self.key_store
    }

    fn crypto(&self) -> &Self::CryptoProvider {
        &self.crypto
    }

    fn rand(&self) -> &Self::RandProvider {
        &self.crypto
    }
}

impl DsProvider {
    pub fn new() -> Self {
        Self {
            crypto: Default::default(),
            key_store: MemoryStorage::default(),
            protocol_version: ProtocolVersion::Mls10,
        }
    }
}

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
    provider: Arc<DsProvider>,
}

impl DeliveryService {
    pub fn new() -> Self {
        Self {
            provider: Arc::new(DsProvider::new()),
        }
    }

    pub fn new_room(&self, group_info: VerifiableGroupInfo) -> Result<Room, CreateRoomError> {
        let ratchet_tree = group_info
            .extensions()
            .ratchet_tree()
            .ok_or(CreateRoomError::MissingRatchetTree)?
            .ratchet_tree()
            .clone();
        let (group, _group_info) = PublicGroup::from_external(
            self.provider.crypto(),
            self.provider.storage(),
            ratchet_tree,
            group_info,
            ProposalStore::new(),
        )?;

        let room = Room {
            group,
            provider: self.provider.clone(),
        };

        Ok(room)
    }
}

/// Room ID is the MLS group ID
#[derive(serde::Serialize, serde::Deserialize)]
pub struct RoomId(Vec<u8>);

pub struct Room {
    group: PublicGroup,
    provider: Arc<DsProvider>,
}

impl Room {}

fn test(message: &[u8]) {
    use tls_codec::Deserialize;
    let message = MlsMessageIn::tls_deserialize_exact(&message).unwrap();
    let crypto = RustCrypto::default();
    match message.extract() {
        openmls::prelude::MlsMessageBodyIn::PublicMessage(public_message_in) => todo!(),
        openmls::prelude::MlsMessageBodyIn::PrivateMessage(private_message_in) => todo!(),
        openmls::prelude::MlsMessageBodyIn::Welcome(welcome) => todo!(),
        openmls::prelude::MlsMessageBodyIn::GroupInfo(verifiable_group_info) => {
            let group_info = verifiable_group_info.verify(&crypto, todo!()).unwrap();
            let data = group_info
                .group_context()
                .extensions()
                .ratchet_tree()
                .unwrap()
                .ratchet_tree()
                .into_verified(todo!(), &crypto, todo!())
                .unwrap();

            ();
        }
        openmls::prelude::MlsMessageBodyIn::KeyPackage(key_package_in) => todo!(),
    }
}
