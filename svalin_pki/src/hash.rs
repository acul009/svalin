use anyhow::{anyhow, Ok, Result};
use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, Params,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Serialize, Deserialize)]
pub struct ArgonParams {
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    salt: String,
}

impl ArgonParams {
    pub fn basic() -> Self {
        let salt = SaltString::generate(&mut OsRng);

        Self {
            m_cost: 32 * 1024,
            t_cost: 2,
            p_cost: 4,
            salt: salt.as_str().to_owned(),
        }
    }

    pub fn strong() -> Self {
        let salt = SaltString::generate(&mut OsRng);

        Self {
            m_cost: 128 * 1024,
            t_cost: 2,
            p_cost: 8,
            salt: salt.as_str().to_owned(),
        }
    }

    pub async fn derive_key(&self, secret: Vec<u8>) -> Result<Vec<u8>> {
        let params = self.get_params().map_err(|err| anyhow!(err))?;
        let argon = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x10, params);

        let (send, recv) = tokio::sync::oneshot::channel::<Result<Vec<u8>>>();
        let salt_bytes = self.salt.as_bytes().to_owned();

        tokio::task::spawn_blocking(move || {
            debug!("hashing password in blocking task");
            let mut hash = vec![0u8; 32];
            let result = argon
                .hash_password_into(&secret, &salt_bytes, &mut hash)
                .map_err(|err| anyhow!(err));
            debug!("hashing complete");

            if let Err(err) = result {
                send.send(Err(err)).unwrap();
            } else {
                send.send(Ok(hash)).unwrap();
            };
        });

        let hash_result = recv.await?;

        debug!("received hash result from blocking task");
        hash_result
    }

    pub async fn derive_password_hash(self, secret: Vec<u8>) -> Result<PasswordHash> {
        let hash = self.derive_key(secret).await?;
        Ok(PasswordHash { params: self, hash })
    }

    fn get_params(&self) -> Result<Params, argon2::Error> {
        Params::new(self.m_cost, self.t_cost, self.p_cost, None)
    }
}

#[derive(Serialize, Deserialize)]
pub struct PasswordHash {
    params: ArgonParams,
    hash: Vec<u8>,
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
