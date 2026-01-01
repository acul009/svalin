use anyhow::{Result, anyhow};
use aucpace::{AuCPaceClient, ClientMessage};
use curve25519_dalek::{RistrettoPoint, Scalar};
use password_hash::{ParamsString, rand_core::OsRng};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use svalin_pki::{
    ArgonCost, Certificate, CreateCertificateError, CreateCredentialsError, Credential,
    EncryptError, EncryptedCredential, ExportedPublicKey, KeyPair, RootCertificate, Sha512,
    UnverifiedCertificate, argon2::Argon2,
};

use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError, SessionWriteError},
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use totp_rs::TOTP;
use tracing::debug;

use crate::server::user_store::{UserStore, serde_paramsstring};

pub struct ServerInitSuccess {
    pub credential: Credential,
    pub root: RootCertificate,
}

#[derive(Serialize, Deserialize)]
pub struct InitRequest {
    server_cert: UnverifiedCertificate,
    encrypted_credential: EncryptedCredential,
    totp_secret: TOTP,
    /// The username of the user being added
    username: Vec<u8>,

    /// The salt used when computing the verifier
    secret_exponent: Scalar,

    /// The password hasher's parameters used when computing the verifier
    #[serde(with = "serde_paramsstring")]
    params: ParamsString,

    /// The verifier computer from the user's password
    verifier: RistrettoPoint,
}

pub(crate) struct InitHandler {
    pool: SqlitePool,
    channel: tokio::sync::Mutex<Option<oneshot::Sender<ServerInitSuccess>>>,
}

impl InitHandler {
    pub fn new(channel: oneshot::Sender<ServerInitSuccess>, pool: SqlitePool) -> Self {
        Self {
            pool,
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
        debug!("incoming init request");
        let mut guard = self.channel.lock().await;

        if guard.is_none() {
            return Err(anyhow!("Already initialized"));
        }

        let keypair = KeyPair::generate();
        let public_key = keypair.export_public_key();
        session.write_object(&public_key).await?;

        let init_request: InitRequest = session.read_object().await?;
        let root = init_request
            .encrypted_credential
            .certificate()
            .clone()
            .use_as_root()?;
        let my_credential = keypair.upgrade(init_request.server_cert)?;

        UserStore::add_root_user(
            &self.pool,
            init_request.username,
            init_request.encrypted_credential,
            init_request.totp_secret,
            init_request.secret_exponent,
            init_request.params,
            init_request.verifier,
        )
        .await?;

        debug!("init request handled");

        let Some(channel) = guard.take() else {
            return Err(anyhow!("channel not found"));
        };

        session
            .write_object::<std::result::Result<(), ()>>(&Ok(()))
            .await?;

        let _: Result<(), SessionReadError> = session.read_object().await;

        let _ = channel.send(ServerInitSuccess {
            credential: my_credential,
            root,
        });

        Ok(())
    }

    fn key() -> String {
        "init".to_owned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
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
    EncryptRootError(#[from] EncryptError),
    #[error("server sent error status back")]
    ServerError,
}

pub struct ClientInitSuccess {
    pub root_credential: Credential,
    pub server_cert: Certificate,
}

pub struct Init {
    root: Credential,
    username: Vec<u8>,
    password: Vec<u8>,
    totp: totp_rs::TOTP,
}

impl Init {
    pub fn new(
        totp: totp_rs::TOTP,
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
        debug!("sending init request");

        // Create server certificate

        let public_key: ExportedPublicKey = session
            .read_object()
            .await
            .map_err(InitError::ReadRequestError)?;
        let server_cert: Certificate = self
            .root
            .create_server_certificate_for_key(&public_key)
            .map_err(InitError::CreateCertError)?;

        // create aucpace login info

        let mut pace_client = AuCPaceClient::<Sha512, Argon2, OsRng, 16>::new(OsRng);

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

        let encrypted_credential = self.root.export(self.password).await?;

        let init_request = InitRequest {
            username: self.username.clone(),
            totp_secret: self.totp.clone(),
            encrypted_credential,
            params,
            secret_exponent,
            server_cert: server_cert.clone().to_unverified(),
            verifier,
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

        debug!("init completed");
        match server_result {
            Ok(()) => Ok(ClientInitSuccess {
                root_credential: self.root.clone(),
                server_cert: server_cert,
            }),
            Err(_) => Err(InitError::ServerError),
        }
    }
}
