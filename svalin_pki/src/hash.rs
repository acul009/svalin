use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, Params,
};
use serde::{Deserialize, Serialize};

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
            m_cost: 128 * 1024,
            t_cost: 2,
            p_cost: 4,
            salt: salt.as_str().to_owned(),
        }
    }

    pub fn strong() -> Self {
        let salt = SaltString::generate(&mut OsRng);
        Self {
            m_cost: 1024 * 1024,
            t_cost: 5,
            p_cost: 8,
            salt: salt.as_str().to_owned(),
        }
    }

    pub fn derive_key(&self, secret: &[u8]) -> Result<Vec<u8>> {
        let params = self.get_params().map_err(|err| anyhow!(err))?;
        let argon = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x10, params);
        let mut hash = vec![0u8; 32];
        argon
            .hash_password_into(secret, self.salt.as_bytes(), &mut hash)
            .map_err(|err| anyhow!(err))?;

        Ok(hash)
    }

    fn get_params(&self) -> Result<Params, argon2::Error> {
        Params::new(self.m_cost, self.t_cost, self.p_cost, None)
    }
}
