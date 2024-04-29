use anyhow::Result;
use flutter_rust_bridge::frb;
pub use svalin::client::{Client, FirstConnect, Init, Login};

#[frb(external)]
impl Client {
    pub fn get_profiles() -> Result<Vec<String>> {}
    pub async fn first_connect(address: String) -> Result<FirstConnect> {}
}

pub fn test_submodule() {
    println!("Hello, world!");
}

#[frb(mirror(FirstConnect))]
pub enum _FirstConnect {
    Init(Init),
    Login(Login),
}

#[frb(external)]
impl Init {
    pub async fn init(&self) -> Result<()> {}
}

#[frb(external)]
impl Login {
    pub async fn login(&self) -> Result<()> {}
}
