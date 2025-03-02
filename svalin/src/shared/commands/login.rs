use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::ArgonParams;
use svalin_rpc::rpc::{command::handler::CommandHandler, session::Session};
use tokio_util::sync::CancellationToken;

use crate::server::user_store::UserStore;

#[derive(Serialize, Deserialize)]
struct LoginAttempt {
    password_hash: Vec<u8>,
    current_totp: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginSuccess {
    pub encrypted_credentials: Vec<u8>,
}

pub struct LoginHandler {
    user_store: UserStore,
    fake_seed: Vec<u8>,
}

impl LoginHandler {
    pub fn new(user_store: UserStore, fake_seed: Vec<u8>) -> Self {
        Self {
            user_store,
            fake_seed,
        }
    }
}

#[async_trait]
impl CommandHandler for LoginHandler {
    type Request = ();

    fn key() -> String {
        "login".to_string()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        _cancel: CancellationToken,
    ) -> Result<()> {
        let username: String = session.read_object().await?;

        let user = self.user_store.get_user_by_username(&username)?;

        let hash_params = match &user {
            Some(user) => user.client_hash_options.clone(),
            None => {
                let mut seed = self.fake_seed.clone();
                seed.extend_from_slice(username.as_bytes());
                ArgonParams::strong_with_fake_salt(&seed)
            }
        };

        session.write_object(&hash_params).await?;

        let attempt: LoginAttempt = session.read_object().await?;

        let password_match = match &user {
            Some(user) => {
                user.password_double_hash
                    .verify(attempt.password_hash)
                    .await?
            }
            None => {
                let mut seed = self.fake_seed.clone();
                seed.extend_from_slice(username.as_bytes());
                ArgonParams::basic_with_fake_salt(&seed)
                    .derive_key(attempt.password_hash)
                    .await?;
                false
            }
        };

        let user_data = if password_match {
            let user = user.unwrap();

            if user.totp_secret.check_current(&attempt.current_totp)? {
                Some(LoginSuccess {
                    encrypted_credentials: user.encrypted_credentials,
                })
            } else {
                None
            }
        } else {
            None
        };

        session.write_object(&user_data).await?;

        Ok(())
    }
}
