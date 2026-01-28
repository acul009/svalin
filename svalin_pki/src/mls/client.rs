use std::sync::Arc;

use crate::{
    Certificate,
    mls::{
        key_package::{KeyPackage, KeyPackageError},
        provider::{PostcardCodec, SvalinProvider},
    },
};
use openmls::{
    group::{
        AddMembersError, GroupId, MlsGroup, NewGroupError, OutgoingWireFormatPolicy,
        PURE_CIPHERTEXT_WIRE_FORMAT_POLICY, WIRE_FORMAT_POLICIES,
    },
    prelude::{Ciphersuite, CredentialWithKey, KeyPackageNewError, ProtocolVersion},
};
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;
use tokio::task::JoinError;

use crate::Credential;

pub struct MlsClient {
    provider: Arc<SvalinProvider>,
    svalin_credential: Credential,
    mls_credential_with_key: CredentialWithKey,
}

impl MlsClient {
    pub fn new(
        credential: Credential,
        storage_provider: SqliteStorageProvider<PostcardCodec>,
    ) -> Self {
        let public_info = CredentialWithKey {
            credential: credential.get_certificate().into(),
            signature_key: credential.get_certificate().public_key().into(),
        };
        Self {
            provider: Arc::new(SvalinProvider::new(storage_provider)),
            svalin_credential: credential,
            mls_credential_with_key: public_info,
        }
    }

    fn ciphersuite(&self) -> Ciphersuite {
        // ChaCha20 icompatible with rust crypto
        Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519
    }

    pub fn provider(&self) -> &SvalinProvider {
        &self.provider
    }

    pub fn protocol_version(&self) -> ProtocolVersion {
        self.provider.protocol_version()
    }

    pub async fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        let cipher_suite = self.ciphersuite();
        let provider = self.provider.clone();
        let svalin_credential = self.svalin_credential.clone();
        let mls_credential_with_key = self.mls_credential_with_key.clone();
        let mls_key_package = tokio::task::spawn_blocking(move || {
            let provider = provider;
            openmls::prelude::KeyPackage::builder().build(
                cipher_suite,
                provider.as_ref(),
                &svalin_credential,
                mls_credential_with_key,
            )
        })
        .await??
        .key_package()
        .clone();
        let key_package = KeyPackage::new(
            self.svalin_credential.get_certificate().clone(),
            mls_key_package,
        )?;

        Ok(key_package)
    }

    pub async fn create_device_group(
        &self,
        device: KeyPackage,
        other_members: Vec<KeyPackage>,
    ) -> Result<GroupCreationInfo, CreateDeviceGroupError> {
        // I'm kind of in over my head again. So I'll try fighting my way through here in very small steps with a few comments.

        // So the first step would be getting a list of all members for the group.
        // These should be:
        // - The target device
        // - The current user
        // - The current users sessions
        // - The root user
        // - The root users sessions
        //
        // And I just now notized, that I already need to have this info then creating this group.
        // So now I gotta think about how to add these to the parameters nicely

        let mut group = MlsGroup::builder()
            .ciphersuite(self.provider().ciphersuite())
            .with_group_id(GroupId::from_slice(device.spki_hash().as_slice()))
            // No idea yet if this prevents the creation of public groups.
            // If it does, I need to change it so the server can actually track group members.
            .with_wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
            .build(
                self.provider(),
                &self.svalin_credential,
                self.mls_credential_with_key.clone(),
            )?;

        let mls_key_packages = other_members
            .into_iter()
            .chain([device].into_iter())
            .map(|key_package| key_package.unpack())
            .collect::<Vec<_>>();

        let (_, welcome, _) = group.add_members(
            self.provider(),
            &self.svalin_credential,
            mls_key_packages.as_slice(),
        )?;

        let ratchet_tree = group.export_ratchet_tree();

        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateKeyPackageError {
    #[error("error trying to create mls key package: {0}")]
    KeyPackageNewError(#[from] KeyPackageNewError),
    #[error("error trying to serialize mls key package: {0}")]
    SerializationError(#[from] tls_codec::Error),
    #[error("error trying to create mls key package: {0}")]
    KeyPackageError(#[from] KeyPackageError),
    #[error("error trying to join tokio blocking task: {0}")]
    JoinError(#[from] JoinError),
}

#[derive(Debug, thiserror::Error)]
pub enum CreateDeviceGroupError {
    #[error("error trying to create mls group: {0}")]
    NewGroupError(
        #[from] NewGroupError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
    ),
    #[error("error trying to add members to mls group: {0}")]
    AddMembersError(
        #[from]
        AddMembersError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
    ),
}

pub struct GroupCreationInfo {}
