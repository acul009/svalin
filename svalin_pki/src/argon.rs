use anyhow::Result;
use argon2::{Argon2, Params, password_hash::ParamsString};
use rand::Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ArgonCost {
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
}

impl ArgonCost {
    pub fn strong() -> Self {
        Self {
            m_cost: 128 * 1024,
            t_cost: 2,
            p_cost: 8,
        }
    }

    pub fn basic() -> Self {
        Self {
            m_cost: 32 * 1024,
            t_cost: 2,
            p_cost: 4,
        }
    }

    pub fn get_params(&self) -> Params {
        Params::new(self.m_cost, self.t_cost, self.p_cost, None).unwrap()
    }

    pub fn get_argon_hasher<'a>(&self) -> Argon2<'a> {
        Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x10,
            self.get_params(),
        )
    }
}

#[derive(Error, Debug)]
pub enum ParamsStringParseError {
    #[error("m_cost missing or invalid")]
    MCostUnavailable,
    #[error("t_cost missing or invalid")]
    TCostUnavailable,
    #[error("p_cost missing or invalid")]
    PCostUnavailable,
}

impl TryFrom<ParamsString> for ArgonCost {
    type Error = ParamsStringParseError;

    fn try_from(value: ParamsString) -> std::result::Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl TryFrom<&ParamsString> for ArgonCost {
    type Error = ParamsStringParseError;

    fn try_from(value: &ParamsString) -> std::result::Result<Self, Self::Error> {
        let m_cost = value
            .get_decimal("m")
            .ok_or(ParamsStringParseError::MCostUnavailable)?;
        let t_cost = value
            .get_decimal("t")
            .ok_or(ParamsStringParseError::TCostUnavailable)?;
        let p_cost = value
            .get_decimal("p")
            .ok_or(ParamsStringParseError::PCostUnavailable)?;

        Ok(Self {
            m_cost,
            t_cost,
            p_cost,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ArgonParams {
    cost: ArgonCost,
    salt: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum DeriveKeyError {
    #[error("error hashing password: {0}")]
    HashError(argon2::Error),
    #[error("error joining task: {0}")]
    JoinError(tokio::task::JoinError),
}

impl ArgonParams {
    fn random_salt() -> Vec<u8> {
        rand::rng()
            .sample_iter(rand::distr::StandardUniform)
            .take(32)
            .collect()
    }

    fn fake_salt(seed: &[u8]) -> Vec<u8> {
        let hash = ring::digest::digest(&ring::digest::SHA512_256, seed);

        hash.as_ref().to_vec()
    }

    pub fn strong() -> Self {
        Self::strong_with_salt(Self::random_salt())
    }

    pub fn strong_with_fake_salt(seed: &[u8]) -> Self {
        Self::strong_with_salt(Self::fake_salt(seed))
    }

    fn strong_with_salt(salt: Vec<u8>) -> Self {
        Self {
            cost: ArgonCost::strong(),
            salt: salt,
        }
    }

    pub fn basic() -> Self {
        Self::basic_with_salt(Self::random_salt())
    }

    pub fn basic_with_fake_salt(seed: &[u8]) -> Self {
        Self::basic_with_salt(Self::fake_salt(seed))
    }

    fn basic_with_salt(salt: Vec<u8>) -> Self {
        Self {
            cost: ArgonCost::basic(),
            salt,
        }
    }

    pub async fn derive_key(&self, mut secret: Vec<u8>) -> Result<[u8; 32], DeriveKeyError> {
        let argon = self.cost.get_argon_hasher();

        let salt_bytes = self.salt.as_slice().to_owned();

        let result = tokio::task::spawn_blocking(move || {
            // debug!("running blocking task");
            let mut hash = [0u8; 32];
            let result = argon
                .hash_password_into(&secret, &salt_bytes, &mut hash)
                .map(move |_| hash)
                .map_err(DeriveKeyError::HashError);

            secret.zeroize();

            result
        })
        .await
        .map_err(DeriveKeyError::JoinError)?;

        result
    }

    pub async fn derive_password_hash(self, secret: Vec<u8>) -> Result<PasswordHash> {
        let hash = self.derive_key(secret).await?;
        Ok(PasswordHash { params: self, hash })
    }

    pub fn get_argon_hasher<'a>(&self) -> Argon2<'a> {
        self.cost.get_argon_hasher()
    }
}

#[derive(Serialize, Deserialize)]
pub struct PasswordHash {
    params: ArgonParams,
    hash: [u8; 32],
}

impl PasswordHash {
    pub async fn verify(&self, secret: Vec<u8>) -> Result<bool> {
        let hash = self.params.derive_key(secret).await?;
        Ok(self.hash == hash)
    }
}

#[cfg(test)]
mod test {

    #[tokio::test]
    async fn test_hash() {
        let password = "testpass".as_bytes().to_owned();

        let params = super::ArgonParams::basic();
        let hashed = params.derive_password_hash(password.clone()).await.unwrap();

        assert!(hashed.verify(password).await.unwrap());
    }

    #[tokio::test]
    async fn stress_test() {
        let password = "testpass".as_bytes().to_owned();

        let mut joinset = tokio::task::JoinSet::new();
        for _ in 0..10 {
            let pw = password.clone();
            joinset.spawn(async move {
                let _ = super::ArgonParams::strong().derive_key(pw).await.unwrap();
            });
        }

        for _ in 0..50 {
            let pw = password.clone();
            joinset.spawn(async move {
                let _ = super::ArgonParams::basic().derive_key(pw).await.unwrap();
            });
        }

        while let Some(res) = joinset.join_next().await {
            res.unwrap();
        }
    }
}
