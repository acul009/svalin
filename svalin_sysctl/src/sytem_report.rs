use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemReport {
    pub generated_at: u64,
    pub os: OS,
}

impl SystemReport {
    pub async fn create() -> anyhow::Result<Self> {
        #[cfg(windows)]
        let os = OS::Windows;

        #[cfg(unix)]
        let os = OS::Linux;

        #[cfg(all(not(unix), not(windows)))]
        let os = OS::Unknown;

        let generated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs();

        Ok(Self { os, generated_at })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum OS {
    Windows,
    Linux,
    Unknown,
}
