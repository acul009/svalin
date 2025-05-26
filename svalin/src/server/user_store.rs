use std::{fmt::Debug, sync::Arc};

use anyhow::Result;
use aucpace::StrongDatabase;
use curve25519_dalek::{RistrettoPoint, Scalar};
use password_hash::ParamsString;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use svalin_pki::{Certificate, EncryptedCredentials};
use totp_rs::TOTP;
use tracing::{debug, instrument};

#[derive(Serialize, Deserialize)]
pub struct StoredUser {
    pub certificate: Certificate,
    pub encrypted_credentials: EncryptedCredentials,
    pub totp_secret: TOTP,
    /// The username of whoever is registering
    pub username: Vec<u8>,

    /// The salt used when computing the verifier
    pub secret_exponent: Scalar,

    /// The password hasher's parameters used when computing the verifier
    #[serde(with = "serde_paramsstring")]
    pub params: ParamsString,

    /// The verifier computer from the user's password
    pub verifier: RistrettoPoint,
}

#[derive(Debug)]
pub struct UserStore {
    pool: sqlx::SqlitePool,
}

impl UserStore {
    pub fn open(pool: SqlitePool) -> Arc<Self> {
        Arc::new(Self { pool })
    }

    pub async fn get_user(&self, fingerprint: &[u8; 32]) -> Result<Option<StoredUser>> {
        let fingerprint = fingerprint.to_vec();
        let user_data = sqlx::query!("SELECT data FROM users WHERE fingerprint = ?", fingerprint)
            .fetch_optional(&self.pool)
            .await?;
        match user_data {
            None => Ok(None),
            Some(user_data) => Ok(Some(postcard::from_bytes(&user_data.data)?)),
        }
    }

    pub async fn get_user_by_username(&self, username: &[u8]) -> Result<Option<StoredUser>> {
        let user_data = sqlx::query!("SELECT data FROM users WHERE username = ?", username)
            .fetch_optional(&self.pool)
            .await?;
        match user_data {
            None => Ok(None),
            Some(user_data) => Ok(Some(postcard::from_bytes(&user_data.data)?)),
        }
    }

    #[instrument(skip_all)]
    pub async fn add_user(
        &self,
        certificate: Certificate,
        username: Vec<u8>,
        encrypted_credentials: EncryptedCredentials,
        totp_secret: TOTP,
        secret_exponent: Scalar,
        params: ParamsString,
        verifier: RistrettoPoint,
    ) -> Result<()> {
        let user = StoredUser {
            certificate,
            username,
            encrypted_credentials,
            totp_secret,
            secret_exponent,
            params,
            verifier,
        };

        debug!("requesting user update transaction");

        let fingerprint = user.certificate.fingerprint().to_vec();

        let userdata = postcard::to_extend(&user, Vec::new())?;

        sqlx::query!(
            "INSERT INTO users (fingerprint, username, data) VALUES (?, ?, ?)",
            fingerprint,
            user.username,
            userdata
        )
        .execute(&self.pool)
        .await?;

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
        let user = tokio::runtime::Handle::current()
            .block_on(self.get_user_by_username(username))
            .map_err(|err| tracing::error!("{}", err))
            .ok()??;

        Some((user.verifier, user.secret_exponent, user.params))
    }

    fn store_verifier_strong(
        &mut self,
        _username: &[u8],
        _uad: Option<&[u8]>,
        _verifier: Self::PasswordVerifier,
        _secret_exponent: Self::Exponent,
        _params: ParamsString,
    ) {
        unimplemented!();
    }
}

pub mod serde_paramsstring {
    use core::fmt;
    use password_hash::ParamsString;
    use serde::de::{Error, Visitor};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(data: &ParamsString, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(data.as_str())
    }

    struct ParamsStringVisitor {}

    impl<'de> Visitor<'de> for ParamsStringVisitor {
        type Value = ParamsString;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a valid PHC parameter string")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            v.parse().map_err(Error::custom)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ParamsString, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ParamsStringVisitor {})
    }
}
