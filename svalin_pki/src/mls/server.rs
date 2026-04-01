use std::collections::HashSet;

use openmls::prelude::{ProtocolVersion, SignaturePublicKey, Verifiable, group_info};
use openmls_rust_crypto::RustCrypto;
use openmls_sqlx_storage::SqliteStorageProvider;

use crate::{
    Certificate, SpkiHash, Verifier, get_current_timestamp,
    mls::{
        group_id::{ParseGroupIdError, SvalinGroupId},
        key_package::{KeyPackage, KeyPackageError, UnverifiedKeyPackage},
        provider::PostcardCodec,
        public_processor::{self, AddGroupError, ProcessMessageError, PublicProcessorHandle},
        transport_types::{
            MessageToSend, MessageToServer, MessageToServerTransport, NewGroup, NewGroupTransport,
        },
    },
};

pub struct MlsServer<Verifier, KeyRetriever> {
    processor: PublicProcessorHandle,
    verifier: Verifier,
    key_retriever: KeyRetriever,
    crypto: RustCrypto,
    protocol_version: ProtocolVersion,
}

impl<Verifier, KeyRetriever> MlsServer<Verifier, KeyRetriever>
where
    Verifier: crate::Verifier,
    KeyRetriever: crate::mls::key_retriever::KeyRetriever,
{
    pub fn new(
        storage_provider: SqliteStorageProvider<PostcardCodec>,
        verifier: Verifier,
        key_retriever: KeyRetriever,
    ) -> Self {
        let processor = PublicProcessorHandle::new(storage_provider);
        let crypto = RustCrypto::default();
        let protocol_version = ProtocolVersion::default();

        Self {
            processor,
            verifier,
            key_retriever,
            crypto,
            protocol_version,
        }
    }

    pub async fn verify_key_package(
        &self,
        key_package: UnverifiedKeyPackage,
        // This one is here to allow verifying to an exact certificate on upload, so noone uploads keypackages that don't belong to them
        verifier: &impl crate::Verifier,
    ) -> Result<KeyPackage, KeyPackageError> {
        key_package
            .verify(&self.crypto, self.protocol_version, verifier)
            .await
    }

    async fn add_svalin_group(
        &self,
        new_group: NewGroup,
    ) -> Result<MessageToSend, AddDeviceGroupError<KeyRetriever::Error>> {
        // I somehow need to inspect this group without creating it, but that means I have to manually verify it and therefore get the public key myself...
        //
        // So I found 2 ways to do this:
        // - Either I gather the public key by hand, which might be doable from just getting the peer certificate from the session
        //      Update: this doesn't work, since even then I can't access the ratchet tree to get the members
        // - Or I just create the group, inspect it and then delete it if it's not up to my expectations.
        //      Update: I did almost that, instead I just create a MemoryStorage and just drop it right after creating the group

        let temp_group = self.processor.check_group(new_group.clone()).await?;

        let members = temp_group
            .members()
            .map(|member| member.credential.deserialized::<SpkiHash>())
            .collect::<Result<HashSet<_>, _>>()?;

        let id = SvalinGroupId::from_group_id(temp_group.group_id())?;
        let required_members = self
            .key_retriever
            .get_required_group_members(&id)
            .await
            .map_err(AddDeviceGroupError::KeyRetrieverError)?
            .into_iter()
            .collect::<HashSet<_>>();

        if members != required_members {
            return Err(todo!());
        }

        let to_send = self.processor.add_group(new_group.clone()).await?;

        Ok(to_send)
    }

    pub async fn process_message(
        &self,
        message: MessageToServerTransport,
    ) -> Result<MessageToSend, ProcessError<KeyRetriever::Error>> {
        let message = message.unpack()?;
        match message {
            MessageToServer::GroupMessage(data) => self
                .processor
                .process_message(data)
                .await
                .map_err(Into::into),
            MessageToServer::NewDeviceGroup { device_group } => self
                .add_device_group(device_group)
                .await
                .map_err(Into::into),
        }
    }

    async fn add_device_group(
        &self,
        new_group: NewGroup,
    ) -> Result<MessageToSend, AddDeviceGroupError<KeyRetriever::Error>> {
        let svalin_id = SvalinGroupId::from_group_id(new_group.group_info.group_id())?;
        match &svalin_id {
            SvalinGroupId::DeviceGroup(_spki_hash) => {
                // no additional things to do here
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(AddDeviceGroupError::UnexpectedGroup);
            }
        }

        self.add_svalin_group(new_group).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError<KeyRetrieverError> {
    #[error("tls codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("message error: {0}")]
    MessageError(#[from] public_processor::ProcessMessageError),
    #[error("add device group error: {0}")]
    AddDeviceGroupError(#[from] AddDeviceGroupError<KeyRetrieverError>),
}

#[derive(Debug, thiserror::Error)]
pub enum AddDeviceGroupError<KeyRetrieverError> {
    #[error("tls codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("error adding group: {0}")]
    AddGroupError(#[from] AddGroupError),
    #[error("error parsing group id: {0}")]
    ParseGroupIdError(#[from] ParseGroupIdError),
    #[error("key retriever error: {0}")]
    KeyRetrieverError(#[source] KeyRetrieverError),
    #[error("expected a different group")]
    UnexpectedGroup,
}
