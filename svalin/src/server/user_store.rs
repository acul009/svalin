use std::{fmt::Debug, sync::Arc};

use anyhow::{anyhow, Ok, Result};
use serde::{Deserialize, Serialize};
use svalin_pki::{ArgonParams, Certificate, PasswordHash};
use totp_rs::TOTP;
use tracing::{debug, instrument};

#[derive(Serialize, Deserialize)]
pub struct StoredUser {
    certificate: Certificate,
    username: String,
    encrypted_credentials: Vec<u8>,
    client_hash_options: ArgonParams,
    password_double_hash: PasswordHash,
    totp_secret: TOTP,
}

#[derive(Debug)]
pub struct UserStore {
    scope: marmelade::Scope,
}

impl UserStore {
    pub fn open(scope: marmelade::Scope) -> Arc<Self> {
        Arc::new(Self { scope })
    }

    fn get_user(&self, fingerprint: &[u8; 32]) -> Result<Option<StoredUser>> {
        let mut user: Option<StoredUser> = None;

        self.scope.view(|b| {
            let b = b.get_bucket("userdata")?;
            user = b.get_object(fingerprint)?;

            Ok(())
        })?;

        Ok(user)
    }

    fn get_user_by_username(&self, username: &str) -> Result<Option<StoredUser>> {
        let mut user: Option<StoredUser> = None;

        self.scope.view(|b| {
            let usernames = b.get_bucket("usernames")?;

            let public_key_user: Option<Vec<u8>> = usernames.get_object(username)?;

            if let Some(public_key) = public_key_user {
                let b = b.get_bucket("userdata")?;
                user = b.get_object(&public_key)?;
            }

            Ok(())
        })?;

        Ok(user)
    }

    #[instrument(skip_all)]
    pub async fn add_user(
        &self,
        certificate: Certificate,
        username: String,
        encrypted_credentials: Vec<u8>,
        client_hash: [u8; 32],
        client_hash_options: ArgonParams,
        totp_secret: TOTP,
    ) -> Result<()> {
        let user = StoredUser {
            certificate,
            username,
            encrypted_credentials,
            client_hash_options,
            password_double_hash: ArgonParams::basic()
                .derive_password_hash(client_hash.to_vec())
                .await?,
            totp_secret,
        };

        debug!("requesting user update transaction");

        self.scope.update(move |b| {
            let fingerprint = user.certificate.get_fingerprint().to_vec();

            let usernames = b.get_or_create_bucket("usernames")?;

            if usernames.get_kv(&user.username).is_some() {
                return Err(anyhow!("Username already in use"));
            }

            let b = b.get_or_create_bucket("userdata")?;
            if b.get_kv(&fingerprint).is_some() {
                return Err(anyhow!("User with uuid already exists"));
            }

            b.put_object(fingerprint.to_owned(), &user)?;

            usernames.put(user.username.clone(), fingerprint)?;

            Ok(())
        })?;

        debug!("user successfully added");

        Ok(())
    }
}
