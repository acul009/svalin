use serde::{Deserialize, Serialize};

pub mod bitlocker;

use bitlocker::BitLockerVolume;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Windows {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitlocker_volumes: Option<Vec<BitLockerVolume>>,
}

impl Windows {
    pub async fn create() -> Option<Self> {
        if cfg!(not(target_os = "windows")) {
            return None;
        }

        Some(Self {
            bitlocker_volumes: bitlocker::query_volumes().await,
        })
    }
}
