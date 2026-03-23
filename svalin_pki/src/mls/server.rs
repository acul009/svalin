use openmls::prelude::{ProtocolVersion, Verifiable};
use openmls_rust_crypto::RustCrypto;
use openmls_sqlx_storage::SqliteStorageProvider;

use crate::{
    Certificate, Verifier,
    mls::{
        key_package::{KeyPackage, KeyPackageError, UnverifiedKeyPackage},
        provider::PostcardCodec,
        public_processor::{AddGroupError, PublicProcessorHandle},
        transport_types::{NewGroup, NewGroupTransport},
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

    pub async fn add_device_group(
        &self,
        device: Certificate,
        new_group: NewGroupTransport,
    ) -> Result<(), AddDeviceGroupError> {
        // I somehow need to inspect this group without creating it, but that means I have to manually verify it and therefore get the public key myself...
        //
        // So I found 2 ways to do this:
        // - Either I gather the public key by hand, which might be doable from just getting the peer certificate from the session
        // - Or I just create the group, inspect it and then delete it if it's not up to my expectations.
        let new_group = new_group.unpack()?;

        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AddDeviceGroupError {
    #[error("tls codec error: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("error adding group: {0}")]
    AddGroupError(#[from] AddGroupError),
}
