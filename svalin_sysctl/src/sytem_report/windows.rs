use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Windows {}

impl Windows {
    pub async fn create() -> anyhow::Result<Option<Self>> {
        if cfg!(not(target_os = "windows")) {
            return Ok(None);
        }

        Ok(Some(Self {}))
    }
}
