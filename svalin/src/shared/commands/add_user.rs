use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use aucpace::{AuCPaceClient, ClientMessage};
use curve25519_dalek::{RistrettoPoint, Scalar};
use password_hash::{ParamsString, rand_core::OsRng};
use serde::{Deserialize, Serialize};
use svalin_pki::{
    ArgonCost, Certificate, EncryptedCredentials, PermCredentials, argon2::Argon2, sha2::Sha512,
};
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::{Session, SessionReadError},
};
use tokio_util::sync::CancellationToken;
use totp_rs::TOTP;
use tracing::{debug, error, instrument};

use crate::{
    permissions::Permission,
    server::user_store::{UserStore, serde_paramsstring},
};

#[derive(Serialize, Deserialize)]
pub struct AddUserRequest {
    certificate: Certificate,
    // username: String,
    encrypted_credentials: EncryptedCredentials,
    totp_secret: TOTP,
    current_totp: String,
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

impl From<&PermissionPrecursor<AddUserHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<AddUserHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

#[derive(Serialize, Deserialize, Debug, thiserror::Error)]
pub enum AddUserError {
    #[error("TOTP and Secret didn't match")]
    TotpMismatch,
    #[error("An undisclosed Error occured")]
    Generic,
}

#[derive(Debug)]
pub struct AddUserHandler {
    userstore: Arc<UserStore>,
}

impl AddUserHandler {
    pub fn new(userstore: Arc<UserStore>) -> Self {
        Self { userstore }
    }
}

#[async_trait]
impl CommandHandler for AddUserHandler {
    type Request = AddUserRequest;

    fn key() -> String {
        "add_user".to_owned()
    }

    #[must_use]
    #[instrument(skip_all)]
    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        _: CancellationToken,
    ) -> anyhow::Result<()> {
        debug!("request received, performing checks");

        let actual_current_totp = request.totp_secret.generate_current()?;

        if actual_current_totp != request.current_totp {
            session
                .write_object::<Result<(), AddUserError>>(&Err(AddUserError::TotpMismatch))
                .await?;
            return Err(AddUserError::TotpMismatch.into());
        }

        debug!("totp check successful");

        let add_result = self
            .userstore
            .add_user(
                request.certificate,
                request.username,
                request.encrypted_credentials,
                request.totp_secret,
                request.secret_exponent,
                request.params,
                request.verifier,
            )
            .await;

        if let Err(err) = add_result {
            error!("error adding user: {}", err);
            session
                .write_object::<Result<(), AddUserError>>(&Err(AddUserError::Generic))
                .await?;
        }

        debug!("requested user was added to the user store");

        session
            .write_object::<Result<(), AddUserError>>(&Ok(()))
            .await?;

        Ok(())
    }
}

pub struct AddUser {
    request: AddUserRequest,
}

impl AddUser {
    pub async fn new(
        credentials: &PermCredentials,
        username: Vec<u8>,
        password: Vec<u8>,
        totp_secret: TOTP,
    ) -> Result<Self> {
        let mut pace_client = AuCPaceClient::<Sha512, Argon2, OsRng, 16>::new(OsRng);

        let hasher = ArgonCost::strong().get_argon_hasher();

        let (username, secret_exponent, params, verifier) = match pace_client
            .register_alloc_strong(&username, password.clone(), hasher.params().clone(), hasher)
            .map_err(|err| anyhow!(err))?
        {
            ClientMessage::StrongRegistration {
                username,
                secret_exponent,
                params,
                verifier,
            } => (username, secret_exponent, params, verifier),
            _ => {
                return Err(anyhow!("unexpected message"));
            }
        };

        debug!("hash params: {}", &params);

        debug!("requesting user to be added");

        let certificate = credentials.get_certificate().to_owned();
        debug!("certificate extracted");

        let encrypted_credentials = credentials.export(password.clone()).await?;
        debug!("credentials encrypted");

        let request = AddUserRequest {
            certificate,
            username: username.to_vec(),
            encrypted_credentials,
            current_totp: totp_secret.generate_current()?,
            totp_secret: totp_secret,
            params: params,
            secret_exponent: secret_exponent,
            verifier: verifier,
        };

        Ok(Self { request })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AddUserDispatcherError {
    #[error("failed to receive response from server: {0}")]
    ReadResponseError(SessionReadError),
    #[error("error from server: {0}")]
    FromServer(AddUserError),
}

impl CommandDispatcher for AddUser {
    type Output = ();
    type Request = AddUserRequest;
    type Error = AddUserDispatcherError;

    fn key() -> String {
        AddUserHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &self.request
    }

    async fn dispatch(self, session: &mut Session) -> Result<(), Self::Error> {
        debug!("waiting on confirmation for new user");

        let success: Result<(), AddUserError> = session
            .read_object()
            .await
            .map_err(AddUserDispatcherError::ReadResponseError)?;

        debug!("received answer");

        success.map_err(AddUserDispatcherError::FromServer)?;

        debug!("user confirmed as successfully added");

        Ok(())
    }
}
