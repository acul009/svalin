use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, RwLock},
};

use openmls::{
    group::{
        AddMembersError, CommitBuilderStageError, CreateCommitError, CreateMessageError,
        MergeCommitError, MergePendingCommitError, MlsGroup, NewGroupError, ProcessMessageError,
        StagedWelcome, WelcomeError,
    },
    prelude::{
        Ciphersuite, CredentialType, CredentialWithKey, KeyPackage, KeyPackageBundle,
        KeyPackageNewError, MlsMessageIn, MlsMessageOut, ProcessedMessageContent, ProtocolMessage,
        SenderRatchetConfiguration, Welcome,
    },
};
use openmls_rust_crypto::{MemoryStorage, MemoryStorageError, RustCrypto};
use openmls_traits::{OpenMlsProvider, signatures::SignerError};

use crate::{
    Certificate, Credential, DecryptError, EncryptError, EncryptedObject, signed_message::CanSign,
};

impl From<&Certificate> for openmls::credentials::Credential {
    fn from(value: &Certificate) -> Self {
        // While there is the X509 credential type, it is not yet supported my openmls.
        // For now we'll have to use basic and handle verification ourselfves
        Self::new(CredentialType::Basic, value.to_der().to_vec())
    }
}

impl openmls_traits::signatures::Signer for Credential {
    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, SignerError> {
        Ok(self.borrow_keypair().sign(payload).as_ref().to_vec())
    }

    fn signature_scheme(&self) -> openmls::prelude::SignatureScheme {
        openmls::prelude::SignatureScheme::ED25519
    }
}

#[derive(Debug)]
pub struct SvalinProvider {
    crypto: RustCrypto,
    key_store: MemoryStorage,
}

impl OpenMlsProvider for SvalinProvider {
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

#[derive(Debug)]
pub struct MlsClient {
    provider: Arc<SvalinProvider>,
    credential: Credential,
    public_info: CredentialWithKey,
    cipher_suite: Ciphersuite,
}

impl MlsClient {
    pub fn new(credential: Credential) -> Self {
        let public_info = CredentialWithKey {
            credential: credential.get_certificate().into(),
            signature_key: credential.get_certificate().public_key().into(),
        };
        // ChaCha20 icompatible with rust crypto
        let cipher_suite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
        Self {
            provider: Arc::new(SvalinProvider {
                crypto: Default::default(),
                key_store: MemoryStorage::default(),
            }),
            credential,
            public_info,
            cipher_suite,
        }
    }

    pub async fn export(
        &self,
        password: Vec<u8>,
    ) -> Result<EncryptedObject<HashMap<Vec<u8>, Vec<u8>>>, EncryptError> {
        let values = self.provider.key_store.values.read().unwrap();

        let encrypted = EncryptedObject::encrypt_with_password(values.deref(), password).await?;

        Ok(encrypted)
    }

    pub async fn import(
        credential: Credential,
        encrypted: EncryptedObject<HashMap<Vec<u8>, Vec<u8>>>,
        password: Vec<u8>,
    ) -> Result<Self, DecryptError> {
        let decrypted = encrypted.decrypt_with_password(password).await?;

        let key_store = MemoryStorage {
            values: RwLock::new(decrypted),
        };

        let public_info = CredentialWithKey {
            credential: credential.get_certificate().into(),
            signature_key: credential.get_certificate().public_key().into(),
        };
        let cipher_suite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

        Ok(Self {
            provider: Arc::new(SvalinProvider {
                crypto: Default::default(),
                key_store,
            }),
            credential,
            public_info,
            cipher_suite,
        })
    }

    pub fn create_group(&self) -> Result<Group, NewGroupError<MemoryStorageError>> {
        let group = MlsGroup::builder()
            .use_ratchet_tree_extension(true)
            .sender_ratchet_configuration(Group::ratchet_config())
            .build(
                self.provider.deref(),
                &self.credential,
                self.public_info.clone(),
            )?;

        Ok(Group {
            group,
            provider: self.provider.clone(),
            credential: self.credential.clone(),
        })
    }

    pub fn join_group(&self, welcome: Welcome) -> Result<Group, GroupError> {
        let staged_join = StagedWelcome::new_from_welcome(
            self.provider.deref(),
            &Group::join_config(),
            welcome,
            None,
        )?;

        let mut group = staged_join.into_group(self.provider.deref())?;
        group.merge_pending_commit(self.provider.deref())?;

        Ok(Group {
            group,
            provider: self.provider.clone(),
            credential: self.credential.clone(),
        })
    }

    pub(crate) fn create_key_package(&self) -> Result<KeyPackageBundle, KeyPackageNewError> {
        KeyPackage::builder().build(
            self.cipher_suite,
            self.provider.deref(),
            &self.credential,
            self.public_info.clone(),
        )
    }
}

mod group_defaults;

pub struct Group {
    group: MlsGroup,
    provider: Arc<SvalinProvider>,
    credential: Credential,
}

#[derive(Debug, thiserror::Error)]
pub enum GroupError {
    #[error("Failed to add members: {0}")]
    AddMembersError(#[from] AddMembersError<MemoryStorageError>),
    #[error("Failed to create commit: {0}")]
    CreateCommitError(#[from] CreateCommitError),
    #[error("Failed to stage commit: {0}")]
    StageCommitError(#[from] CommitBuilderStageError<MemoryStorageError>),
    #[error("Failed to merge pending commit: {0}")]
    MergePendingCommitError(#[from] MergePendingCommitError<MemoryStorageError>),
    #[error("Failed to merge commit: {0}")]
    MergeCommitError(#[from] MergeCommitError<MemoryStorageError>),
    #[error("Failed to welcome: {0}")]
    WelcomeError(#[from] WelcomeError<MemoryStorageError>),
    #[error("Failed to create message: {0}")]
    CreateMessageError(#[from] CreateMessageError),
    #[error("Failed to process message: {0}")]
    ProcessMessageError(#[from] ProcessMessageError),
    #[error("Failed to store cipherdata: {0}")]
    StoreError(#[from] MemoryStorageError),
}

impl Group {
    pub fn add_members(
        &mut self,
        key_packages: &[KeyPackage],
    ) -> Result<(MlsMessageOut, MlsMessageOut), GroupError> {
        let (message, welcome, _) =
            self.group
                .add_members(self.provider.deref(), &self.credential, key_packages)?;

        self.group.merge_pending_commit(self.provider.deref())?;

        Ok((message, welcome))
    }

    /// This creates a new commit as well as the requested message right afterwards.
    /// The commit here helps to ensure that the message can easily be read, as the per message ratchet
    /// is kept in memory only and reading the last message would require all prior messages of that epoch too
    pub fn create_message(&mut self, message: &[u8]) -> Result<[MlsMessageOut; 2], GroupError> {
        let empty_commit = self
            .group
            .commit_builder()
            .load_psks(self.provider.storage())?
            .build(
                self.provider.rand(),
                self.provider.crypto(),
                &self.credential,
                |_| false,
            )?
            .stage_commit(self.provider.deref())?
            .into_commit();

        self.group.merge_pending_commit(self.provider.deref())?;

        let message =
            self.group
                .create_message(self.provider.deref(), &self.credential, message)?;

        Ok([empty_commit, message])
    }

    pub fn process_message(
        &mut self,
        message: ProtocolMessage,
    ) -> Result<Option<Vec<u8>>, GroupError> {
        let content = self
            .group
            .process_message(self.provider.deref(), message)?
            .into_content();

        match content {
            ProcessedMessageContent::ApplicationMessage(application_message) => {
                Ok(Some(application_message.into_bytes()))
            }
            ProcessedMessageContent::ProposalMessage(queued_proposal) => {
                // Todo: check proposal validity

                self.group
                    .store_pending_proposal(self.provider.storage(), *queued_proposal)?;
                Ok(None)
            }
            ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
                // Todo: check staged commit validity

                self.group
                    .merge_staged_commit(self.provider.deref(), *staged_commit)?;
                Ok(None)
            }
            ProcessedMessageContent::ExternalJoinProposalMessage(queued_proposal) => todo!(),
        }
    }
}
