use std::time::Duration;

use anyhow::{Context, Ok, Result};
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};
use svalin_rpc::verifiers::skip_verify::SkipServerVerification;
use tracing::{debug, instrument};

use crate::shared::commands::add_user::AddUser;
use crate::shared::commands::init;
use crate::shared::commands::public_server_status::GetPutblicStatus;
use crate::{
    client::verifiers::upstream_verifier::UpstreamVerifier,
    shared::commands::public_server_status::PublicStatus,
};

use super::Client;

impl Client {
    #[instrument]
    pub async fn first_connect(address: String) -> Result<FirstConnect> {
        debug!("try connecting to {address}");

        let client = RpcClient::connect(&address, None, SkipServerVerification::new()).await?;

        debug!("successfully connected");

        let conn = client.upstream_connection();

        debug!("requesting public status");

        let server_status = conn.dispatch(GetPutblicStatus).await?;

        debug!("public status: {server_status:?}");

        let first_connect = match server_status {
            PublicStatus::WaitingForInit => FirstConnect::Init(Init { client, address }),
            PublicStatus::Ready => FirstConnect::Login(Login { client }),
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

impl Init {
    #[instrument(skip_all)]
    pub async fn init(
        self,
        username: String,
        password: String,
        totp_secret: totp_rs::TOTP,
    ) -> Result<()> {
        let (root, server_cert) = self
            .client
            .upstream_connection()
            .dispatch(init::Init)
            .await
            .context("failed to initialize server certificate")?;

        self.client.close();

        tokio::time::sleep(Duration::from_secs(3)).await;

        let verifier = UpstreamVerifier::new(root.get_certificate().clone(), server_cert.clone());

        let client = RpcClient::connect(&self.address, Some(&root), verifier)
            .await
            .context("failed to connect to server after certificate initialization")?;
        let connection = client.upstream_connection();

        debug!("connected to server with certificate");

        connection
            .dispatch(AddUser {
                credentials: &root,
                username: username.clone(),
                password: password.clone().into(),
                totp_secret: totp_secret,
            })
            .await
            .context("failed to add root user")?;
        // connection
        //     .add_user(
        //         &root,
        //         username.clone(),
        //         password.clone().into(),
        //         totp_secret,
        //     )
        //     .await
        //     .context("failed to add root user")?;

        Client::add_profile(
            username,
            self.address,
            server_cert,
            root.get_certificate().clone(),
            root,
            password.into(),
        )
        .await
        .context("failed to save profile")?;

        Ok(())
    }
}

pub struct Login {
    client: RpcClient,
}

impl Login {
    pub async fn login(&self) -> Result<()> {
        todo!()
    }
}
