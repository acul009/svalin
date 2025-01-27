use std::{fmt::Display, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{ArgonParams, Certificate, PermCredentials};
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::Session,
};
use tokio_util::sync::CancellationToken;
use totp_rs::TOTP;
use tracing::{debug, error, instrument};

use crate::{permissions::Permission, server::user_store::UserStore};

#[derive(Serialize, Deserialize, Clone)]
pub struct AddUserRequest {
    certificate: Certificate,
    username: String,
    encrypted_credentials: Vec<u8>,
    client_hash: [u8; 32],
    client_hash_options: ArgonParams,
    totp_secret: TOTP,
    current_totp: String,
}

impl From<&PermissionPrecursor<AddUserRequest, AddUserHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<AddUserRequest, AddUserHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

#[derive(Serialize, Deserialize, Debug)]
enum AddUserError {
    TotpMismatch,
    Generic,
}

impl Display for AddUserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddUserError::TotpMismatch => write!(f, "TOTP and Secret didn't match"),
            AddUserError::Generic => write!(f, "An undisclosed Error occured"),
        }
    }
}

impl std::error::Error for AddUserError {}

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
                request.client_hash,
                request.client_hash_options,
                request.totp_secret,
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
        username: String,
        password: Vec<u8>,
        totp_secret: TOTP,
    ) -> Result<Self> {
        let client_hash_options = ArgonParams::strong();

        debug!("requesting user to be added");

        let certificate = credentials.get_certificate().to_owned();
        debug!("certificate extracted");

        let encrypted_credentials = credentials.to_bytes(password.clone()).await?;
        debug!("credentials encrypted");

        let client_hash = client_hash_options.derive_key(password.clone()).await?;
        debug!("password hash created");

        let request = AddUserRequest {
            certificate,
            username: username,
            encrypted_credentials,
            client_hash,
            client_hash_options,
            current_totp: totp_secret.generate_current()?,
            totp_secret: totp_secret,
        };

        Ok(Self { request })
    }
}

#[async_trait]
impl CommandDispatcher for AddUser {
    type Output = ();
    type Request = AddUserRequest;

    fn key() -> String {
        AddUserHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        self.request.clone()
    }

    async fn dispatch(self, session: &mut Session, _: Self::Request) -> Result<()> {
        debug!("waiting on confirmation for new user");

        let success: Result<(), AddUserError> = session.read_object().await?;

        debug!("received answer");

        success?;

        debug!("user confirmed as successfully added");

        Ok(())
    }
}
