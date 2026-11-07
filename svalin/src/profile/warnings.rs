use svalin_pki::{SpkiHash, get_current_timestamp, mls::SvalinGroupId};
use svalin_sysctl::system_report::windows::bitlocker::{self, ProtectionStatus, VolumeStatus};

use super::persistent;

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

    pub fn update_device_warnings(&mut self, persistent: &persistent::State, device: &SpkiHash) {
        let warnings = self.generate_device_warnings(persistent, device);
        self.update_device(device, warnings);
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

    fn generate_device_warnings(
        &self,
        persistent: &persistent::State,
        spki_hash: &SpkiHash,
    ) -> Vec<Device> {
        let mut warnings = Vec::new();
        if let Some(report) = persistent.report(spki_hash.clone()) {
            for disk in &report.system_report.disks {
                let part_free = disk.available_space as f32 / disk.total_space as f32;
                if part_free < 0.1 {
                    warnings.push(Device::DiskSpaceLow {
                        disk: disk.mount_point.clone(),
                        free: disk.available_space,
                        total: disk.total_space,
                    });
                }
            }
            let extensions = &report.system_report.extensions;
            if let Some(windows) = extensions.windows() {
                self.generate_windows_warnings(&mut warnings, windows);
            }
            if let Some(pve) = extensions.proxmox_ve() {
                self.generate_proxmox_ve_warnings(&mut warnings, pve);
            }
            if let Some(pmg) = extensions.proxmox_mg() {
                if pmg.attachment_quarantine_count > 0 {
                    warnings.push(Device::PMGAttachmentQuarantine(
                        pmg.attachment_quarantine_count,
                    ));
                }
                if pmg.virus_quarantine_count > 0 {
                    warnings.push(Device::PMGVirusQuarantine(pmg.virus_quarantine_count));
                }
            }
        }

        if let Some(meta_info) = persistent.meta_info(spki_hash.clone()) {
            if meta_info.name.is_empty() {
                warnings.push(Device::MissingName)
            }
        } else {
            warnings.push(Device::MissingName)
        }

        if let Some(error) = persistent.group_error(&SvalinGroupId::DeviceGroup(spki_hash.clone()))
        {
            warnings.push(Device::DeviceGroupBroken(error.to_owned()));
        }

        warnings
    }

    fn generate_proxmox_ve_warnings(
        &self,
        warnings: &mut Vec<Device>,
        pve: &svalin_sysctl::system_report::proxmox_ve::ProxmoxVE,
    ) {
        for backup_job in pve.backup_jobs.iter().flatten() {
            if backup_job.is_overdue(get_current_timestamp()) {
                warnings.push(Device::BackupOverdue {
                    name: backup_job.id.clone(),
                    due_at: backup_job.due_at.expect("already checked in is_overdue"),
                });
                continue;
            }
            match &backup_job.status {
                svalin_sysctl::system_report::proxmox_ve::backup::BackupStatus::Failed {
                    finished_at,
                    message,
                } => {
                    warnings.push(Device::BackupFailed {
                        name: backup_job.id.clone(),
                        message: message.clone(),
                        finished_at: *finished_at,
                    });
                }
                _ => (),
            }
        }
    }

    fn generate_windows_warnings(
        &self,
        warnings: &mut Vec<Device>,
        windows: &svalin_sysctl::system_report::windows::Windows,
    ) {
        if let Some(bitlocker) = &windows.bitlocker_volumes {
            for drive in bitlocker {
                if drive.volume_status == VolumeStatus::FullyDecrypted
                    && drive.protection_status == ProtectionStatus::Off
                    && drive.encryption_percentage == 0
                {
                    continue;
                }
                warnings.push(Device::BitlockerActive {
                    drive: drive.mount_point.clone(),
                    status: drive.volume_status.clone(),
                    protection: drive.protection_status.clone(),
                    percentage: drive.encryption_percentage,
                });
            }
        }
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
