use std::{fmt::Display, ops::Add, sync::Arc};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_macros::rpc_dispatch;
use svalin_pki::{ArgonParams, Certificate, PermCredentials};
use svalin_rpc::{Session, SessionOpen};
use totp_rs::TOTP;
use tracing::{debug, field::debug, instrument, span, Instrument, Level};

use crate::server::users::{StoredUser, UserStore};

use super::public_server_status::PublicStatus;

#[derive(Serialize, Deserialize)]
struct AddUserRequest {
    certificate: Certificate,
    username: String,
    encrypted_credentials: Vec<u8>,
    client_hash: Vec<u8>,
    client_hash_options: ArgonParams,
    totp_secret: TOTP,
    current_totp: String,
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

fn add_user_key() -> String {
    "add_user".to_owned()
}

#[async_trait]
impl svalin_rpc::CommandHandler for AddUserHandler {
    fn key(&self) -> String {
        add_user_key()
    }

    #[must_use]
    #[instrument]
    async fn handle(&self, mut session: Session<SessionOpen>) -> anyhow::Result<()> {
        debug!("reading request to add user");
        let request: AddUserRequest = session.read_object().await?;

        debug!("request received, performing checks");

        let actual_current_totp = request.totp_secret.generate_current()?;

        if actual_current_totp != request.current_totp {
            session
                .write_object::<Result<(), AddUserError>>(&Err(AddUserError::TotpMismatch))
                .await?;
            return Err(AddUserError::TotpMismatch.into());
        }

        debug!("totp check successful");

        let add_result = self.userstore.add_user(
            request.certificate,
            request.username,
            request.encrypted_credentials,
            request.client_hash,
            request.client_hash_options,
            request.totp_secret,
        );

        if let Err(err) = add_result {
            session
                .write_object::<Result<(), AddUserError>>(&Err(AddUserError::Generic))
                .await?;
        }

        session
            .write_object::<Result<(), AddUserError>>(&Ok(()))
            .await?;
        Ok(())
    }
}

#[instrument(skip_all)]
#[rpc_dispatch(add_user_key())]
pub async fn add_user(
    session: &mut Session<SessionOpen>,
    credentials: PermCredentials,
    username: String,
    password: &[u8],
    totp_secret: TOTP,
) -> Result<()> {
    let client_hash_options = ArgonParams::strong();

    debug!("requesting user to be added");

    let certificate = credentials.get_certificate().to_owned();
    debug!("certificate extracted");

    let encrypted_credentials = credentials.to_bytes(password)?;
    debug!("credentials encrypted");

    let client_hash = client_hash_options.derive_key(password)?;
    debug!("password hash created");

    let request = AddUserRequest {
        certificate,
        username,
        encrypted_credentials,
        client_hash,
        client_hash_options: client_hash_options,
        current_totp: totp_secret.generate_current()?,
        totp_secret,
    };

    debug!("user request ready to be added");

    session
        .write_object(&request)
        .instrument(span!(Level::TRACE, "write add user request"))
        .await?;

    debug!("waiting on confirmation for new user");

    let success: Result<(), AddUserError> = session.read_object().await?;

    success?;

    Ok(())
}
