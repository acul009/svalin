use std::fmt::Debug;
use std::time::Duration;

use anyhow::{Context, Result};
use svalin_pki::{DecodeCredentialsError, ExactVerififier, KnownCertificateVerifier};
use svalin_rpc::rpc::command::dispatcher::DispatcherError;
use svalin_rpc::rpc::connection::ConnectionDispatchError;
use svalin_rpc::rpc::session::SessionDispatchError;
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};
use svalin_rpc::verifiers::skip_verify::SkipServerVerification;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

use crate::server::INIT_SERVER_SHUTDOWN_COUNTDOWN;
use crate::shared::commands::login::LoginDispatcherError;
use crate::shared::commands::public_server_status::GetPutblicStatus;
use crate::shared::commands::public_server_status::PublicStatus;
use crate::shared::commands::{self, init};

use super::Client;

impl Client {
    #[instrument]
    pub async fn first_connect(address: String) -> Result<FirstConnect> {
        debug!("try connecting to {address}");

        let client = RpcClient::connect(
            &address,
            None,
            SkipServerVerification::new(),
            CancellationToken::new(),
        )
        .await?;
        debug!("successfully connected");

        let conn = client.upstream_connection();

        debug!("requesting public status");

        let server_status = conn.dispatch(GetPutblicStatus).await?;

        debug!("public status: {server_status:?}");

        let first_connect = match server_status {
            PublicStatus::WaitingForInit => FirstConnect::Init(Init { client, address }),
            PublicStatus::Ready => FirstConnect::Login(Login { client, address }),
        };

        debug!("returning from first connect");

        Ok(first_connect)
    }
}

pub enum FirstConnect {
    Init(Init),
    Login(Login),
}

pub struct Init {
    client: RpcClient,
    address: String,
}

impl Debug for Init {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Init").finish()
    }
}

impl Init {
    #[instrument(skip_all)]
    pub async fn init(
        self,
        username: String,
        password: Vec<u8>,
        totp_secret: totp_rs::TOTP,
    ) -> Result<String> {
        let init_data = self
            .client
            .upstream_connection()
            .dispatch(init::Init::new(
                totp_secret.clone(),
                username.clone().into_bytes(),
                password.clone(),
            )?)
            .await
            .context("failed to initialize server certificate")?;

        self.client.close(Duration::from_secs(1)).await?;

        tokio::time::sleep(INIT_SERVER_SHUTDOWN_COUNTDOWN).await;

        let client = RpcClient::connect(
            &self.address,
            None,
            ExactVerififier::new(init_data.server_cert).to_tls_verifier(),
            CancellationToken::new(),
        )
        .await?;

        let login = Login {
            address: self.address,
            client,
        };

        Ok(login
            .login(username, password, totp_secret.generate_current()?)
            .await?)
    }

    pub fn address(&self) -> &str {
        &self.address
    }
}

pub struct Login {
    client: RpcClient,
    address: String,
}

impl Debug for Login {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Login").finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("failed to dispatch login command")]
    DispatchError(#[from] ConnectionDispatchError<commands::login::LoginDispatcherError>),
    #[error("wrong password")]
    WrongPassword,
    #[error("invalid totp")]
    InvalidTotp,
    #[error("failed to decode credentials")]
    DecodeCredentialsError(#[from] DecodeCredentialsError),
    #[error("failed to add profile")]
    AddProfileError(#[from] anyhow::Error),
}

impl Login {
    pub async fn login(
        self,
        username: String,
        password: Vec<u8>,
        totp: String,
    ) -> Result<String, LoginError> {
        let login_data = self
            .client
            .upstream_connection()
            .dispatch(commands::login::Login {
                username: username.clone().into_bytes(),
                password: password.clone(),
                totp,
            })
            .await
            .map_err(|err| match err {
                ConnectionDispatchError::DispatchError(SessionDispatchError::DispatcherError(
                    DispatcherError::Other(LoginDispatcherError::WrongPassword),
                )) => LoginError::WrongPassword,
                ConnectionDispatchError::DispatchError(SessionDispatchError::DispatcherError(
                    DispatcherError::Other(LoginDispatcherError::InvalidTotp),
                )) => LoginError::InvalidTotp,
                _ => LoginError::DispatchError(err),
            })?;

        let profile = Client::add_profile(
            username,
            self.address,
            login_data.server_cert,
            login_data.root_cert,
            login_data.user_credential,
            login_data.device_credential,
            password,
        )
        .await
        .context("failed to save profile")?;

        Ok(profile)
    }

    pub fn address(&self) -> &str {
        &self.address
    }
}
