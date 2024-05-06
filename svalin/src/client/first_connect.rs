use std::time::Duration;

use anyhow::Result;
use svalin_rpc::SkipServerVerification;

use crate::{
    client::verifiers::upstream_verifier::{self, UpstreamVerifier},
    shared::commands::{
        add_user::add_userDispatcher,
        init::initDispatcher,
        public_server_status::{get_public_statusDispatcher, PublicStatus},
    },
};

use super::Client;

impl Client {
    pub async fn first_connect(address: String) -> Result<FirstConnect> {
        println!("try connecting to {address}");

        let client =
            svalin_rpc::Client::connect(address.clone(), None, SkipServerVerification::new())
                .await?;

        println!("successfully connected");

        let mut conn = client.upstream_connection();

        println!("requesting public status");

        let server_status = conn.get_public_status().await?;

        println!("public status: {server_status:?}");

        let first_connect = match server_status {
            PublicStatus::WaitingForInit => FirstConnect::Init(Init { client, address }),
            PublicStatus::Ready => FirstConnect::Login(Login { client }),
        };

        println!("returning from first connect");

        Ok(first_connect)
    }
}

pub enum FirstConnect {
    Init(Init),
    Login(Login),
}

pub struct Init {
    client: svalin_rpc::Client,
    address: String,
}

impl Init {
    pub async fn init(
        &self,
        username: String,
        password: String,
        totp_secret: totp_rs::TOTP,
    ) -> Result<()> {
        let (root, server_cert) = self.client.upstream_connection().init().await?;

        self.client.close();

        tokio::time::sleep(Duration::from_secs(1)).await;

        let verifier = UpstreamVerifier::new(root.get_certificate().clone(), server_cert);

        let client =
            svalin_rpc::Client::connect(self.address.clone(), Some(&root), verifier).await?;
        let mut connection = client.upstream_connection();

        connection
            .add_user(root, username, password.as_bytes(), totp_secret)
            .await?;

        // save configuration to profile

        todo!()
    }
}

pub struct Login {
    client: svalin_rpc::Client,
}

impl Login {
    pub async fn login(&self) -> Result<()> {
        todo!()
    }
}
