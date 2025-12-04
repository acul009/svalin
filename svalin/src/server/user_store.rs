use std::{fmt::Debug, sync::Arc};

use anyhow::{Result, anyhow};
use aucpace::StrongDatabase;
use curve25519_dalek::{RistrettoPoint, Scalar};
use password_hash::ParamsString;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use svalin_pki::{
    Certificate, CertificateChain, CertificateChainBuilder, CertificateType, EncryptedCredential,
    SpkiHash,
};
use totp_rs::TOTP;

#[derive(Serialize, Deserialize)]
pub struct StoredUser {
    pub encrypted_credential: EncryptedCredential,
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
    root: Certificate,
    pool: sqlx::SqlitePool,
}

impl UserStore {
    pub async fn add_root_user(
        pool: &SqlitePool,
        username: Vec<u8>,
        encrypted_credential: EncryptedCredential,
        totp_secret: TOTP,
        secret_exponent: Scalar,
        params: ParamsString,
        verifier: RistrettoPoint,
    ) -> Result<()> {
        let count: u64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await?;

        if count > 0 {
            return Err(anyhow!("Root user already exists"));
        }

        if encrypted_credential.certificate().certificate_type() != CertificateType::Root {
            return Err(anyhow!("Wrong certificate type for root"));
        }

        let user = StoredUser {
            username,
            encrypted_credential,
            totp_secret,
            secret_exponent,
            params,
            verifier,
        };

        let cert = user.encrypted_credential.certificate();
        let spki_hash = cert.spki_hash().as_slice();
        let username = &user.username;

        let data = postcard::to_stdvec(&user)?;

        sqlx::query!(
            "INSERT INTO users (spki_hash, username, data) VALUES (?, ?, ?)",
            spki_hash,
            username,
            data
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub fn open(pool: SqlitePool, root: Certificate) -> Arc<Self> {
        Arc::new(Self { pool, root })
    }

    pub async fn get_user(&self, spki_hash: &SpkiHash) -> anyhow::Result<Option<StoredUser>> {
        let spki_hash = spki_hash.as_slice();
        let user_data = sqlx::query!("SELECT data FROM users WHERE spki_hash = ?", spki_hash)
            .fetch_optional(&self.pool)
            .await?;
        match user_data {
            None => Ok(None),
            Some(user_data) => Ok(Some(postcard::from_bytes(&user_data.data)?)),
        }
    }

    pub async fn get_user_by_username(
        &self,
        username: &[u8],
    ) -> anyhow::Result<Option<StoredUser>> {
        let user_data = sqlx::query!("SELECT data FROM users WHERE username = ?", username)
            .fetch_optional(&self.pool)
            .await?;
        match user_data {
            None => Ok(None),
            Some(user_data) => Ok(Some(postcard::from_bytes(&user_data.data)?)),
        }
    }

    pub async fn complete_certificate_chain(
        &self,
        mut cert_chain: CertificateChainBuilder,
    ) -> Result<CertificateChain> {
        loop {
            let issuer_spki = cert_chain
                .requested_issuer()
                .expect("chain not finished yet");

            let Some(issuer) = self.get_cert_by_spki_hash(&issuer_spki).await? else {
                return Err(anyhow!("issuer with spki hash {} not found", issuer_spki));
            };

            if let Some(chain) = cert_chain.push_parent(issuer)? {
                return Ok(chain);
            }
        }
    }

    pub async fn get_cert_by_spki_hash(
        &self,
        spki_hash: &SpkiHash,
    ) -> anyhow::Result<Option<Certificate>> {
        let spki_hash = spki_hash.as_slice();
        let user_data =
            sqlx::query_scalar!("SELECT data FROM users WHERE spki_hash = ?", spki_hash)
                .fetch_optional(&self.pool)
                .await?;
        match user_data {
            None => Ok(None),
            Some(user_data) => {
                let user: StoredUser = postcard::from_bytes(&user_data)?;
                Ok(Some(user.encrypted_credential.take_certificate()))
            }
        }
    }

    //     #[instrument(skip_all)]
    //     pub async fn add_user(
    //         &self,
    //         username: Vec<u8>,
    //         encrypted_credentials: EncryptedCredential,
    //         totp_secret: TOTP,
    //         secret_exponent: Scalar,
    //         params: ParamsString,
    //         verifier: RistrettoPoint,
    //     ) -> Result<()> {
    //         let user = StoredUser {
    //             username,
    //             encrypted_credentials,
    //             totp_secret,
    //             secret_exponent,
    //             params,
    //             verifier,
    //         };

    //         debug!("requesting user update transaction");

    //         let certificate = user.encrypted_credentials.certificate();

    //         let fingerprint = certificate.fingerprint();
    //         let fingerprint = fingerprint.as_slice();
    //         let spki_hash = certificate.spki_hash();

    //         let userdata = postcard::to_extend(&user, Vec::new())?;

    //         sqlx::query!(
    //             "INSERT INTO users (fingerprint, spki_hash, username, data) VALUES (?, ?, ?, ?)",
    //             fingerprint,
    //             spki_hash,
    //             user.username,
    //             userdata
    //         )
    //         .execute(&self.pool)
    //         .await?;

    //         Ok(())
    //     }
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
