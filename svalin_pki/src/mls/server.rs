use std::collections::HashSet;

use anyhow::anyhow;
use openmls::prelude::{
    ProtocolVersion, PublicMessageIn, SignaturePublicKey, Verifiable, group_info,
};
use openmls_rust_crypto::RustCrypto;
use openmls_sqlx_storage::SqliteStorageProvider;

use crate::{
    Certificate, SpkiHash, Verifier, get_current_timestamp,
    mls::{
        group_id::{ParseGroupIdError, SvalinGroupId},
        harness::MlsHarness,
        key_package::{KeyPackage, KeyPackageError, UnverifiedKeyPackage},
        provider::PostcardCodec,
        public_processor::{self, AddGroupError, ProcessMessageError, PublicProcessorHandle},
        transport_types::{
            AddToGroup, MessageToSend, MessageToServer, MessageToServerTransport, NewGroup,
            NewGroupTransport,
        },
    },
};

pub struct MlsServer<KeyRetriever, Verifier> {
    harness: MlsHarness<KeyRetriever, Verifier, PublicProcessorHandle>,
}

impl<KeyRetriever, Verifier> MlsServer<KeyRetriever, Verifier>
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

        Self {
            harness: MlsHarness::new(key_retriever, verifier, processor),
        }
    }

    pub async fn verify_key_package(
        &self,
        key_package: UnverifiedKeyPackage,
        // This one is here to allow verifying to an exact certificate on upload, so noone uploads keypackages that don't belong to them
        expected: &SpkiHash,
    ) -> Result<KeyPackage, KeyPackageError> {
        let verified = self.harness.verify_key_package(key_package).await?;
        if verified.spki_hash() != expected {
            return Err(KeyPackageError::SpkiHashMismatch);
        }
        Ok(verified)
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

        let temp_group = self
            .harness
            .processor()
            .check_group(new_group.clone())
            .await?;

        let members = temp_group
            .members()
            .map(|member| member.credential.deserialized::<SpkiHash>())
            .collect::<Result<HashSet<_>, _>>()?;

        let id = SvalinGroupId::from_group_id(temp_group.group_id())?;
        let required_members = self
            .harness
            .key_retriever()
            .get_required_group_members(&id)
            .await
            .map_err(AddDeviceGroupError::KeyRetrieverError)?;

        for required in required_members.iter() {
            if !members.contains(required) {
                return Err(AddDeviceGroupError::MissingMember(required.clone()));
            }
        }

        let to_send = self
            .harness
            .processor()
            .add_group(new_group.clone())
            .await?;

        Ok(to_send)
    }

    pub async fn process_message(
        &self,
        message: MessageToServerTransport,
    ) -> Result<MessageToSend, anyhow::Error> {
        let message = message.unpack()?;
        match message {
            MessageToServer::GroupMessage(data) => self
                .harness
                .processor()
                .process_message(data)
                .await
                .map_err(Into::into),
            MessageToServer::NewDeviceGroup { device_group } => self
                .add_device_group(device_group)
                .await
                .map_err(|err| anyhow!(err)),
            MessageToServer::AddToGroup(add_to_group) => {
                self.handle_add_to_group(add_to_group).await
            }
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

    async fn handle_add_to_group(&self, add_to_group: AddToGroup) -> Result<(), anyhow::Error> {
        let commit = add_to_group.commit;
        let processed = self
            .harness
            .processor()
            .process_message(commit)
            .await
            .map_err(|err| anyhow!(err))?;
        let group_id = processed.group_id()?;

        let ProcessedContent::Commit(commit) = processed.content else {
            anyhow::bail!("Expected a commit message, got {:?}", processed.content)
        };

        if group_id != self.my_device_group {
            anyhow::bail!("received message for unexpected group: {:?}", group_id)
        }

        self.harness.check_commit(&group_id, &commit).await?;

        Ok(())
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
    #[error("missing member: {0}")]
    MissingMember(SpkiHash),
}
