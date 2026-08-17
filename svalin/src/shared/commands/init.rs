use std::sync::Arc;

use anyhow::{Result, anyhow};
use aucpace::{AuCPaceClient, ClientMessage};
use serde::{Deserialize, Serialize};
use svalin_pki::argon2::password_hash::rand_core::OsRng;
use svalin_pki::mls::provider::{ExportedMlsStore, SvalinStorage};
use svalin_pki::{
    ArgonCost, Certificate, CreateCertificateError, CreateCredentialsError, Credential,
    EncryptError, EncryptedCredential, ExportedPublicKey, KeyPair, RootCertificate, Sha512,
    UnverifiedCertificate, argon2::Argon2, serde_paramsstring,
};
use svalin_pki::{ArgonParams, EncryptedObject, UseAsRootError, trust_store};
use svalin_pki::{
    argon2::password_hash::ParamsString,
    curve25519_dalek::{RistrettoPoint, Scalar},
};
use svalin_rpc::transport::aucpace_transport::NONCE_LENGTH;

use async_trait::async_trait;
use svalin_client_store::persistent;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError, SessionWriteError},
};
use svalin_server_store::UserStore;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use totp_rs::Totp;

pub struct ServerInitSuccess {
    pub credential: Credential,
    pub trust_store: trust_store::TrustStore,
}

#[derive(Serialize, Deserialize)]
pub struct InitRequest {
    server_cert: UnverifiedCertificate,
    encrypted_credential: EncryptedCredential,
    credential_key_params: ArgonParams,
    totp_secret: Totp,
    /// The username of the user being added
    username: Vec<u8>,

    /// The salt used when computing the verifier
    secret_exponent: Scalar,

    /// The password hasher's parameters used when computing the verifier
    #[serde(with = "serde_paramsstring")]
    params: ParamsString,

    /// The verifier computer from the user's password
    verifier: RistrettoPoint,

    user_mls_store: ExportedMlsStore,
    persistent_data: EncryptedObject<persistent::State>,
    trust_store: trust_store::Exported,
}

pub(crate) struct InitHandler {
    user_store: Arc<UserStore>,
    channel: tokio::sync::Mutex<Option<oneshot::Sender<ServerInitSuccess>>>,
}

impl InitHandler {
    pub fn new(channel: oneshot::Sender<ServerInitSuccess>, user_store: Arc<UserStore>) -> Self {
        Self {
            user_store,
            channel: tokio::sync::Mutex::new(Some(channel)),
        }
    }
}

#[async_trait]
impl CommandHandler for InitHandler {
    type Request = ();

    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        _: CancellationToken,
    ) -> anyhow::Result<()> {
        tracing::trace!("incoming init request");
        let mut guard = self.channel.lock().await;

        if guard.is_none() {
            return Err(anyhow!("Already initialized"));
        }

        let keypair = KeyPair::generate();
        let public_key = keypair.export_public_key();
        session.write_object(&public_key).await?;

        let init_request: InitRequest = session.read_object().await?;

        let my_credential = keypair.upgrade(init_request.server_cert)?;
        let trust_store = trust_store::TrustStore::import(init_request.trust_store)?;
        if trust_store.root().as_unverified() != init_request.encrypted_credential.certificate() {
            return Err(anyhow!("root mismatch"));
        }

        UserStore::add_root_user(
            &self.user_store,
            init_request.username,
            init_request.encrypted_credential,
            init_request.credential_key_params,
            init_request.totp_secret,
            init_request.secret_exponent,
            init_request.params,
            init_request.verifier,
            init_request.user_mls_store,
            init_request.persistent_data,
        )
        .await?;

        tracing::trace!("init request handled");

        let Some(channel) = guard.take() else {
            return Err(anyhow!("channel not found"));
        };

        session
            .write_object::<std::result::Result<(), ()>>(&Ok(()))
            .await?;

        let _: Result<(), SessionReadError> = session.read_object().await;

        let _ = channel.send(ServerInitSuccess {
            credential: my_credential,
            trust_store,
        });

        Ok(())
    }

    fn key() -> String {
        "init".to_owned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("certificate cannot be used as root: {0}")]
    UseAsRootError(#[from] UseAsRootError),
    #[error("error reading request: {0}")]
    ReadRequestError(SessionReadError),
    #[error("error creating certificate for public key: {0}")]
    CreateCertError(CreateCertificateError),
    #[error("error writing server certificate: {0}")]
    WriteServerCertError(SessionWriteError),
    #[error("error reading success: {0}")]
    ReadSuccessError(SessionReadError),
    #[error("error writing confirm: {0}")]
    WriteConfirmError(SessionWriteError),
    #[error("error with aucpace: {0}")]
    AucPaceError(aucpace::Error),
    #[error("error encrypting root credential: {0}")]
    EncryptError(#[from] EncryptError),
    #[error(transparent)]
    Unspecified(#[from] anyhow::Error),
    #[error("server sent error status back")]
    ServerError,
    #[error("error creating block: {0}")]
    CreateBlockError(#[from] trust_store::CreateBlockError),
}

pub struct ClientInitSuccess {
    pub root_credential: Credential,
    pub trust_store: trust_store::TrustStore,
}

pub struct Init {
    root: Credential,
    username: Vec<u8>,
    password: Vec<u8>,
    totp: totp_rs::Totp,
}

impl Init {
    pub fn new(
        totp: totp_rs::Totp,
        username: Vec<u8>,
        password: Vec<u8>,
    ) -> Result<Self, CreateCredentialsError> {
        let root = Credential::generate_root()?;

        Ok(Self {
            root,
            username,
            password,
            totp,
        })
    }
}

impl CommandDispatcher for Init {
    type Output = ClientInitSuccess;
    type Request = ();
    type Error = InitError;

    fn key() -> String {
        InitHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        tracing::trace!("sending init request");

        // Create server certificate

        let public_key: ExportedPublicKey = session
            .read_object()
            .await
            .map_err(InitError::ReadRequestError)?;
        let server_cert: Certificate = self
            .root
            .create_server_certificate_for_key(&public_key)
            .map_err(InitError::CreateCertError)?;

        // create trust store
        let mut trust_store = trust_store::TrustStore::initialize(
            self.root
                .certificate()
                .clone()
                .to_unverified()
                .use_as_root()?,
        );

        // create aucpace login info

        let mut pace_client = AuCPaceClient::<Sha512, Argon2, OsRng, NONCE_LENGTH>::new(OsRng);

        let hasher = ArgonCost::strong().get_argon_hasher();

        let (_username, secret_exponent, params, verifier) = match pace_client
            .register_alloc_strong(
                &self.username,
                &self.password,
                hasher.params().clone(),
                hasher,
            )
            .map_err(InitError::AucPaceError)?
        {
            ClientMessage::StrongRegistration {
                username,
                secret_exponent,
                params,
                verifier,
            } => (username, secret_exponent, params, verifier),
            _ => {
                unreachable!();
            }
        };

        // send init request
        let credential_key_params = ArgonParams::strong();
        let key = credential_key_params
            .derive_encryption_key(self.password)
            .await?;
        let empty_client_state = persistent::State::empty();
        let persistent_data = EncryptedObject::encrypt(&empty_client_state, &key)?;

        let (_, export_handle) = SvalinStorage::new_memory();
        let user_mls_store = export_handle.export(&key)?;

        let encrypted_credential = self.root.export(&key)?;

        let block = trust_store.add(server_cert.clone(), &self.root)?;
        trust_store.apply(block);

        let init_request = InitRequest {
            username: self.username.clone(),
            totp_secret: self.totp.clone(),
            encrypted_credential,
            credential_key_params,
            params,
            secret_exponent,
            server_cert: server_cert.to_unverified(),
            verifier,
            user_mls_store,
            persistent_data,
            trust_store: trust_store.export(),
        };

        session
            .write_object(&init_request)
            .await
            .map_err(InitError::WriteServerCertError)?;

        let server_result: std::result::Result<(), ()> = session
            .read_object()
            .await
            .map_err(InitError::ReadSuccessError)?;

        session
            .write_object(&())
            .await
            .map_err(InitError::WriteConfirmError)?;

        tracing::trace!("init completed");
        match server_result {
            Ok(()) => Ok(ClientInitSuccess {
                root_credential: self.root.clone(),
                trust_store,
            }),
            Err(_) => Err(InitError::ServerError),
        }
    }
}
