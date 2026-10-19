use svalin_pki::SpkiHash;
use svalin_sysctl::system_report::windows::bitlocker;

#[derive(Clone, Debug)]
pub struct State {
    warnings: Vec<Warning>,
}

impl State {
    pub fn new() -> Self {
        Self {
            warnings: Vec::new(),
        }
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Warning> {
        self.warnings.iter()
    }

    pub fn highest_severity(&self) -> Option<Severity> {
        self.warnings.first().map(Warning::severity)
    }

    pub fn update_device(&mut self, device: &SpkiHash, mut warnings: Vec<Device>) {
        self.warnings.retain(|warning| match warning {
            Warning::Device(spki, _) => spki != device,
        });
        if !warnings.is_empty() {
            warnings.sort_by_key(Device::severity);
            self.warnings
                .push(Warning::Device(device.clone(), warnings));
        }
        self.warnings.sort_by_key(Warning::severity);
    }
}

#[derive(Clone, Debug)]
pub enum Warning {
    Device(SpkiHash, Vec<Device>),
}

impl Warning {
    pub fn severity(&self) -> Severity {
        match self {
            Self::Device(_, warnings) => warnings
                .iter()
                .map(Device::severity)
                .min()
                .unwrap_or(Severity::Low),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug)]
pub enum Device {
    DeviceGroupBroken(String),
    MissingName,
    DiskSpaceLow {
        disk: String,
        free: u64,
        total: u64,
    },
    BitlockerActive {
        drive: String,
        status: bitlocker::VolumeStatus,
        protection: bitlocker::ProtectionStatus,
        percentage: u8,
    },
    PMGAttachmentQuarantine(u64),
    PMGVirusQuarantine(u64),
    BackupOverdue {
        name: String,
        due_at: u64,
    },
    BackupFailed {
        name: String,
        message: String,
        finished_at: Option<u64>,
    },
}

impl Device {
    pub fn severity(&self) -> Severity {
        match self {
            Self::DeviceGroupBroken(_) => Severity::High,
            Self::DiskSpaceLow { .. } => Severity::Medium,
            Self::MissingName => Severity::Low,
            Self::BitlockerActive { .. } => Severity::Medium,
            Self::PMGAttachmentQuarantine(_) => Severity::Low,
            Self::PMGVirusQuarantine(_) => Severity::Low,
            Device::BackupOverdue { .. } => Severity::High,
            Device::BackupFailed { .. } => Severity::High,
        }
    }
}
