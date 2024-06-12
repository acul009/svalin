pub use anyhow::Result;
pub use flutter_rust_bridge::frb;
pub use svalin::client::add_agent::WaitingForConfirmCode;
pub use svalin::client::{Client, FirstConnect, Init, Login};
pub use totp_rs::TOTP;

#[frb(external)]
impl Client {
    pub fn get_profiles() -> Result<Vec<String>> {}
    pub async fn first_connect(address: String) -> Result<FirstConnect> {}
    pub fn remove_profile(profile_key: &str) -> Result<()> {}
    pub async fn open_profile_string(profile_key: String, password: String) -> Result<Client> {}
    pub async fn add_agent_with_code(&self, join_code: String) -> Result<WaitingForConfirmCode> {}
}

pub async fn say_hello() -> Result<String> {
    Ok("Hello, World!".to_owned())
}

#[frb(non_opaque, mirror(FirstConnect))]
pub enum _FirstConnect {
    Init(Init),
    Login(Login),
}

#[frb(external)]
impl Init {
    pub async fn init(self, username: String, password: String, totp_secret: TOTP) -> Result<()> {}
}

#[frb(external)]
impl Login {
    pub async fn login(&self) -> Result<()> {}
}

#[frb(external)]
impl WaitingForConfirmCode {
    pub async fn confirm(self, confirm_code: String) -> Result<()> {}
}
