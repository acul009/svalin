use std::{fmt::Debug, sync::Arc};

use anyhow::{Result, anyhow};
use aucpace::StrongDatabase;
use curve25519_dalek::{RistrettoPoint, Scalar};
use password_hash::ParamsString;
use serde::{Deserialize, Serialize};
use svalin_pki::Certificate;
use totp_rs::TOTP;
use tracing::{debug, instrument};

#[derive(Serialize, Deserialize)]
pub struct StoredUser {
    pub certificate: Certificate,
    pub encrypted_credentials: Vec<u8>,
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

            let public_key_user = usernames.get_kv(username);

            if let Some(public_key) = public_key_user {
                let b = b.get_bucket("userdata")?;
                user = b.get_object(public_key.value())?;
            }

            Ok(())
        })?;

        Ok(user)
    }

    #[instrument(skip_all)]
    pub async fn add_user(
        &self,
        certificate: Certificate,
        username: Vec<u8>,
        encrypted_credentials: Vec<u8>,
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

        self.scope.update(move |b| {
            let fingerprint = user.certificate.fingerprint().to_vec();

            let usernames = b.get_or_create_bucket("usernames")?;

            if usernames.get_kv(&user.username).is_some() {
                return Err(anyhow!("Username already in use"));
            }

            let b = b.get_or_create_bucket("userdata")?;
            if b.get_kv(&fingerprint).is_some() {
                return Err(anyhow!("User with fingerprint already exists"));
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
