pub use std::time::SystemTimeError;

use anyhow::Result;
use flutter_rust_bridge::frb;
pub use totp_rs::{Algorithm, Secret, TOTP};

#[frb(mirror(Algorithm))]
pub enum _Algorithm {
    SHA1,
    SHA256,
    SHA512,
}

pub fn new_totp(account_name: String) -> Result<TOTP> {
    Ok(TOTP::new(
        Algorithm::SHA1,
        8,
        1,
        30,
        Secret::generate_secret().to_bytes()?,
        Some("Svalin".into()),
        account_name,
    )?)
}

#[frb(external)]
impl TOTP {
    pub fn check_current(&self, token: &str) -> Result<bool, SystemTimeError> {}
    pub fn get_url(&self) -> String {}
    pub fn get_qr_png(&self) -> Result<Vec<u8>, String> {}
}
