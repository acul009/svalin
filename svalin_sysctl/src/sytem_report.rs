use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SystemReport {
    os: OS,
}

impl SystemReport {
    pub async fn create() -> anyhow::Result<Self> {
        #[cfg(windows)]
        let os = OS::Windows;

        #[cfg(unix)]
        let os = OS::Linux;

        #[cfg(all(not(unix), not(windows)))]
        let os = OS::Unknown;

        Ok(Self { os })
    }
}

#[derive(Serialize, Deserialize)]
pub enum OS {
    Windows,
    Linux,
    Unknown,
}
