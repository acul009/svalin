use serde::{Deserialize, Serialize};

pub mod bitlocker;
pub mod installed_programs;

use bitlocker::BitLockerVolume;
use installed_programs::InstalledProgram;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Windows {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitlocker_volumes: Option<Vec<BitLockerVolume>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_programs: Option<Vec<InstalledProgram>>,
}

impl Windows {
    pub async fn create() -> Option<Self> {
        if cfg!(not(target_os = "windows")) {
            return None;
        }

        Some(Self {
            bitlocker_volumes: bitlocker::query_volumes().await,
            installed_programs: installed_programs::query_programs().await,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_reports_allow_missing_programs() {
        let report: Windows = serde_json::from_str(r#"{"bitlocker_volumes":[]}"#).unwrap();
        assert!(report.installed_programs.is_none());
        assert_eq!(
            serde_json::to_value(report).unwrap(),
            serde_json::json!({"bitlocker_volumes": []})
        );
    }

    #[test]
    fn populated_programs_round_trip() {
        let report = Windows {
            bitlocker_volumes: None,
            installed_programs: Some(vec![InstalledProgram {
                name: "Example".into(),
                version: Some("1.0".into()),
                publisher: None,
            }]),
        };
        let decoded: Windows =
            serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
        assert_eq!(decoded.installed_programs, report.installed_programs);
    }
}
