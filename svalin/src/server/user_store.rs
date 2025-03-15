use std::{fmt::Debug, sync::Arc};

use anyhow::{Ok, Result, anyhow};
use aucpace::StrongDatabase;
use password_hash::ParamsString;
use serde::{Deserialize, Serialize};
use svalin_pki::{ArgonParams, Certificate, PasswordHash};
use totp_rs::TOTP;
use tracing::{debug, instrument};

#[derive(Serialize, Deserialize)]
pub struct StoredUser {
    pub certificate: Certificate,
    pub username: String,
    pub encrypted_credentials: Vec<u8>,
    pub client_hash_options: ArgonParams,
    pub password_double_hash: PasswordHash,
    pub totp_secret: TOTP,
    pub pake_data: PakeData,
}

#[derive(Serialize, Deserialize)]
pub struct PakeData {
    password_verifier: curve25519_dalek::RistrettoPoint,
    exponent: curve25519_dalek::Scalar,
    params_string: String,
}

#[derive(Debug)]
pub struct UserStore {
    scope: marmelade::Scope,
}

impl UserStore {
    pub fn open(scope: marmelade::Scope) -> Arc<Self> {
        Arc::new(Self { scope })
    }

    pub fn get_user(&self, fingerprint: &[u8; 32]) -> Result<Option<StoredUser>> {
        let mut user: Option<StoredUser> = None;

        self.scope.view(|b| {
            let b = b.get_bucket("userdata")?;
            user = b.get_object(fingerprint)?;

            Ok(())
        })?;

        Ok(user)
    }

    pub fn get_user_by_username(&self, username: &[u8]) -> Result<Option<StoredUser>> {
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
            pake_data: todo!(),
        };

        debug!("requesting user update transaction");

        self.scope.update(move |b| {
            let fingerprint = user.certificate.fingerprint().to_vec();

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

impl StrongDatabase for UserStore {
    type PasswordVerifier = curve25519_dalek::RistrettoPoint;

    type Exponent = curve25519_dalek::Scalar;

    fn lookup_verifier_strong(
        &self,
        username: &[u8],
    ) -> Option<(Self::PasswordVerifier, Self::Exponent, ParamsString)> {
        let user = self
            .get_user_by_username(username)
            .map_err(|err| tracing::error!("{}", err))
            .ok()??;
        let pake = user.pake_data;

        let params = pake
            .params_string
            .parse()
            .map_err(|err| tracing::error!("{}", err))
            .ok()?;

        Some((pake.password_verifier, pake.exponent, params))
    }

    fn store_verifier_strong(
        &mut self,
        username: &[u8],
        uad: Option<&[u8]>,
        verifier: Self::PasswordVerifier,
        secret_exponent: Self::Exponent,
        params: ParamsString,
    ) {
        unimplemented!();
    }
}
