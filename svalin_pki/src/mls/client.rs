use std::sync::Arc;

use crate::{
    CertificateType, UnverifiedCertificate,
    mls::{
        key_package::{KeyPackage, KeyPackageError},
        provider::{PostcardCodec, SvalinProvider},
    },
};
use openmls::{
    framing::errors::MlsMessageError,
    group::{
        AddMembersError, ExportGroupInfoError, GroupId, MergePendingCommitError, MlsGroup,
        MlsGroupJoinConfig, NewGroupError, PURE_CIPHERTEXT_WIRE_FORMAT_POLICY, StagedWelcome,
        WelcomeError,
    },
    prelude::{
        Ciphersuite, CredentialWithKey, KeyPackageNewError, MlsMessageBodyIn, MlsMessageIn,
        ProtocolVersion, RatchetTreeIn, SenderRatchetConfiguration, Welcome,
        group_info::VerifiableGroupInfo,
    },
};
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;
use serde::{Deserialize, Serialize};
use tls_codec::DeserializeBytes;
use tokio::task::JoinError;

use crate::Credential;

pub struct MlsClient {
    // Needs to be an Arc so `spawn_blocking` works.
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
    ) -> Result<DeviceGroupCreationInfo, CreateDeviceGroupError> {
        let provider = self.provider.clone();
        let svalin_credential = self.svalin_credential.clone();
        let credential_with_key = self.mls_credential_with_key.clone();
        tokio::task::spawn_blocking(move || {
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
            //
            // Update: I managed it :D

            let certificate = device.certificate().clone();

            let mut group = MlsGroup::builder()
                .ciphersuite(provider.ciphersuite())
                .with_group_id(GroupId::from_slice(device.spki_hash().as_slice()))
                // No idea yet if this prevents the creation of public groups.
                // If it does, I need to change it so the server can actually track group members.
                .with_wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
                .build(provider.as_ref(), &svalin_credential, credential_with_key)?;

            let mls_key_packages = other_members
                .into_iter()
                .chain([device].into_iter())
                .map(|key_package| key_package.unpack())
                .collect::<Vec<_>>();

            let (_, welcome, _) = group.add_members(
                provider.as_ref(),
                &svalin_credential,
                mls_key_packages.as_slice(),
            )?;

            group.merge_pending_commit(provider.as_ref())?;

            let welcome = welcome.to_bytes()?;

            let group_info = group
                .export_group_info(provider.crypto(), &svalin_credential, true)?
                .to_bytes()?;

            Ok(DeviceGroupCreationInfo {
                certificate: certificate.to_unverified(),
                welcome,
                group_info,
            })
        })
        .await
        .map_err(CreateDeviceGroupError::TokioJoinError)
        .flatten()
    }

    pub async fn join_my_device_group(
        &self,
        group_info: DeviceGroupCreationInfo,
    ) -> Result<(), JoinDeviceGroupError> {
        let provider = self.provider.clone();
        let me = self.svalin_credential.get_certificate().spki_hash().clone();
        let my_parent = self.svalin_credential.get_certificate().issuer().clone();

        let welcome = group_info.welcome()?;

        let ratchet_tree = group_info.ratchet_tree()?;

        let join_config = MlsGroupJoinConfig::builder()
            .max_past_epochs(0)
            .use_ratchet_tree_extension(false)
            .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
            .sender_ratchet_configuration(SenderRatchetConfiguration::new(0, 0))
            .build();

        let welcome = StagedWelcome::new_from_welcome(
            provider.as_ref(),
            &join_config,
            welcome,
            Some(ratchet_tree),
        )?;

        if welcome.group_context().group_id().as_slice() != me.as_slice() {
            return Err(JoinDeviceGroupError::WrongGroupId);
        }

        let creator: UnverifiedCertificate =
            welcome.welcome_sender()?.credential().deserialized()?;

        // Ensure there are only sessions any myself in the group
        welcome
            .members()
            .map(|member| -> Result<(), JoinDeviceGroupError> {
                let certificate: UnverifiedCertificate = member.credential.deserialized()?;
                if certificate.spki_hash() == &me {
                    return Ok(());
                }

                if certificate.certificate_type() != CertificateType::UserDevice {
                    return Err(JoinDeviceGroupError::WrongMemberType);
                }

                Ok(())
            })
            .collect::<Result<(), JoinDeviceGroupError>>()?;

        // TODO: check that members contains root
        if creator.issuer() != &my_parent {
            return Err(JoinDeviceGroupError::WrongGroupCreator);
        }

        let _group = welcome.into_group(provider.as_ref())?;

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct DeviceGroupCreationInfo {
    certificate: UnverifiedCertificate,
    welcome: Vec<u8>,
    group_info: Vec<u8>,
}

impl DeviceGroupCreationInfo {
    pub fn welcome(&self) -> Result<Welcome, GroupCreationUnpackError> {
        let message = MlsMessageIn::tls_deserialize_exact_bytes(&self.welcome.as_slice())?;

        let MlsMessageBodyIn::Welcome(welcome) = message.extract() else {
            return Err(GroupCreationUnpackError::WrongMessageType);
        };

        Ok(welcome)
    }

    pub fn group_info(&self) -> Result<VerifiableGroupInfo, GroupCreationUnpackError> {
        let message = MlsMessageIn::tls_deserialize_exact_bytes(&self.group_info.as_slice())?;

        let MlsMessageBodyIn::GroupInfo(group_info) = message.extract() else {
            return Err(GroupCreationUnpackError::WrongMessageType);
        };

        Ok(group_info)
    }

    pub fn ratchet_tree(&self) -> Result<RatchetTreeIn, GroupCreationUnpackError> {
        let group_info = self.group_info()?;
        let ratchet_tree = group_info
            .extensions()
            .ratchet_tree()
            .ok_or_else(|| GroupCreationUnpackError::MissingRatchetTree)?
            .ratchet_tree()
            .clone();

        Ok(ratchet_tree)
    }

    pub fn certificate(&self) -> &UnverifiedCertificate {
        &self.certificate
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GroupCreationUnpackError {
    #[error("error trying to deserialize mls message: {0}")]
    TlsCoderError(#[from] tls_codec::Error),
    #[error("wrong message type")]
    WrongMessageType,
    #[error("error trying to verify mls signature: {0}")]
    SignatureError(#[from] openmls::prelude::SignatureError),
    #[error("missing ratchet tree extension")]
    MissingRatchetTree,
    #[error("error trying to verify mls ratchet tree: {0}")]
    RatchetTreeError(#[from] openmls::treesync::RatchetTreeError),
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
    #[error("error trying to join task: {0}")]
    TokioJoinError(#[from] tokio::task::JoinError),
}

#[derive(Debug, thiserror::Error)]
pub enum JoinDeviceGroupError {
    #[error("group creation unpack error: {0}")]
    GroupCreationUnpackError(#[from] GroupCreationUnpackError),
    #[error("error while trying to parse welcome: {0}")]
    WelcomeError(
        #[from] WelcomeError<<SvalinProvider as openmls::storage::OpenMlsProvider>::StorageError>,
    ),
    #[error("error in openmls library: {0}")]
    LibraryError(#[from] openmls::error::LibraryError),
    #[error("error in tls codec: {0}")]
    TlsCodecError(#[from] tls_codec::Error),
    #[error("wrong group creator")]
    WrongGroupCreator,
    #[error("wrong group id, not my group")]
    WrongGroupId,
    #[error("wrong member type")]
    WrongMemberType,
}
