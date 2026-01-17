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
        Ciphersuite, CredentialType, CredentialWithKey, KeyPackageBundle, KeyPackageNewError,
        KeyPackageVerifyError, MlsMessageBodyIn, MlsMessageIn, MlsMessageOut, PrivateMessageIn,
        ProcessedMessageContent, ProtocolMessage, Welcome,
    },
};
use openmls_rust_crypto::{MemoryStorage, MemoryStorageError, RustCrypto};
use openmls_traits::signatures::SignerError;
use rand::RngCore;

use crate::{
    Certificate, Credential, DecryptError, EncryptError, EncryptedObject,
    mls::{
        key_package::{KeyPackage, UnverifiedKeyPackage},
        message_types::{Invitation, InvitationError},
    },
    signed_message::CanSign,
};

pub mod client;
pub mod delivery_service;
pub mod key_package;
pub mod message_types;

pub use openmls::prelude::{OpenMlsProvider, ProtocolVersion};

impl From<&Certificate> for openmls::credentials::Credential {
    fn from(value: &Certificate) -> Self {
        // While there is the X509 credential type, it is not yet supported my openmls.
        // For now we'll have to use basic and handle verification ourselfves
        Self::new(CredentialType::Basic, value.as_der().to_vec())
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
