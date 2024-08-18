pub use anyhow::Result;
pub use flutter_rust_bridge::frb;
pub use svalin::client::add_agent::WaitingForConfirmCode;
pub use svalin::client::{device::Device, Client, FirstConnect, Init, Login};
pub use svalin::shared::commands::agent_list::AgentListItem;
pub use svalin::shared::join_agent::PublicAgentData;
pub use svalin_pki::Certificate;
pub use totp_rs::TOTP;
pub mod device;

#[frb(external)]
impl Client {
    pub fn get_profiles() -> Result<Vec<String>> {}
    pub async fn first_connect(address: String) -> Result<FirstConnect> {}
    pub fn remove_profile(profile_key: &str) -> Result<()> {}
    pub async fn open_profile_string(profile_key: String, password: String) -> Result<Client> {}
    pub async fn add_agent_with_code(&self, join_code: String) -> Result<WaitingForConfirmCode> {}
    pub async fn device_list(&self) -> Vec<Device> {}
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
    pub async fn confirm(self, confirm_code: String, agent_name: String) -> Result<()> {}
}