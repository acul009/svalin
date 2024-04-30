use std::{fmt::Display, ops::Add, sync::Arc};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_macros::rpc_dispatch;
use svalin_pki::{ArgonParams, Certificate, PermCredentials};
use svalin_rpc::{Session, SessionOpen};
use totp_rs::TOTP;

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

pub struct AddUserHandler {
    userstore: Arc<UserStore>,
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
    async fn handle(&self, mut session: Session<SessionOpen>) -> anyhow::Result<()> {
        let request: AddUserRequest = session.read_object().await?;

        let actual_current_totp = request.totp_secret.generate_current()?;

        if actual_current_totp != request.current_totp {
            session
                .write_object::<Result<(), AddUserError>>(&Err(AddUserError::TotpMismatch))
                .await?;
            return Err(AddUserError::TotpMismatch.into());
        }

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
                .write_object::<Result<(), AddUserError>>(&Err(AddUserError::TotpMismatch))
                .await?;
        }

        session
            .write_object::<Result<(), AddUserError>>(&Ok(()))
            .await?;
        Ok(())
    }
}

#[rpc_dispatch(add_user_key())]
pub async fn add_user(
    session: &mut Session<SessionOpen>,
    credentials: PermCredentials,
    username: String,
    password: &[u8],
    totp_secret: TOTP,
) -> Result<()> {
    let client_hash_options = ArgonParams::strong();
    let current_totp = totp_secret.generate_current()?;

    let request = AddUserRequest {
        certificate: credentials.get_certificate().to_owned(),
        username,
        encrypted_credentials: credentials.to_bytes(password)?,
        client_hash: client_hash_options.derive_key(password)?,
        client_hash_options: client_hash_options,
        totp_secret,
        current_totp,
    };

    session.write_object(&request).await?;

    let success: Result<(), AddUserError> = session.read_object().await?;

    success?;

    Ok(())
}
