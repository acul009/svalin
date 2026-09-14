use std::{borrow::Cow, collections::HashMap, fmt::Debug};

use serde::{Deserialize, Serialize};
use svalin_pki::{SpkiHash, get_current_timestamp};
use svalin_sysctl::system_report::{
    OSFamily, SystemReport,
    windows::bitlocker::{ProtectionStatus, VolumeStatus},
};

use crate::client::state::warning;

/// This contains the persistent state of the clients available information.
/// It is not meant to contain live information like current cpu usage or online status.
/// This should only contain data which is still relevant after a device has been shut down.
///
/// That also entails that this state should be updated whether the user looks at it or not.
/// Live data, in contrast, should only be updated when the user actively interacts with it - it's ephemeral.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct State {
    pub(crate) devices: HashMap<SpkiHash, DeviceState>,
}

#[derive(Clone, Debug)]
pub enum Update {
    SystemReport(SpkiHash, Report),
    MetaInfo(SpkiHash, MetaInfo),
}

impl Update {
    pub fn affected_device(&self) -> Option<SpkiHash> {
        match self {
            Update::SystemReport(spki_hash, _) => Some(spki_hash.clone()),
            Update::MetaInfo(spki_hash, _) => Some(spki_hash.clone()),
        }
    }
}

impl State {
    pub fn empty() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    pub fn update(&mut self, msg: Update) {
        match msg {
            Update::SystemReport(spki_hash, system_report) => {
                let entry = self.get_device_entry(spki_hash);
                if let Some(report) = &entry.report {
                    if system_report.system_report.generated_at <= report.system_report.generated_at
                    {
                        return;
                    }
                }

                entry.report = Some(system_report);
            }
            Update::MetaInfo(spki_hash, meta_info) => {
                let entry = self.get_device_entry(spki_hash);
                if let Some(meta) = &entry.meta_info {
                    if meta_info.updated_at <= meta.updated_at {
                        return;
                    }
                }

                entry.meta_info = Some(meta_info);
            }
        }
    }

    fn get_device_entry(&mut self, spki_hash: SpkiHash) -> &mut DeviceState {
        self.devices
            .entry(spki_hash.clone())
            .or_insert_with(|| DeviceState {
                spki_hash,
                report: None,
                meta_info: None,
            })
    }

    pub fn devices(&self) -> &HashMap<SpkiHash, DeviceState> {
        &self.devices
    }

    pub(super) fn generate_device_warnings(&self, spki_hash: &SpkiHash) -> Vec<warning::Device> {
        let mut warnings = Vec::new();
        let Some(device) = self.devices.get(spki_hash) else {
            return warnings;
        };
        if let Some(report) = device.report() {
            for disk in &report.system_report.disks {
                let part_free = disk.available_space as f32 / disk.total_space as f32;
                if part_free < 0.1 {
                    warnings.push(warning::Device::DiskSpaceLow {
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
                    warnings.push(warning::Device::PMGAttachmentQuarantine(
                        pmg.attachment_quarantine_count,
                    ));
                }
                if pmg.virus_quarantine_count > 0 {
                    warnings.push(warning::Device::PMGVirusQuarantine(
                        pmg.virus_quarantine_count,
                    ));
                }
            }
        }

        if let Some(meta_info) = device.meta_info() {
            if meta_info.name.is_empty() {
                warnings.push(warning::Device::MissingName)
            }
        } else {
            warnings.push(warning::Device::MissingName)
        }

        warnings
    }

    fn generate_proxmox_ve_warnings(
        &self,
        warnings: &mut Vec<warning::Device>,
        pve: &svalin_sysctl::system_report::proxmox_ve::ProxmoxVE,
    ) {
        for backup_job in pve.backup_jobs.iter().flatten() {
            if backup_job.is_overdue(get_current_timestamp()) {
                warnings.push(warning::Device::BackupOverdue {
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
                    warnings.push(warning::Device::BackupFailed {
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
        warnings: &mut Vec<warning::Device>,
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
                warnings.push(warning::Device::BitlockerActive {
                    drive: drive.mount_point.clone(),
                    status: drive.volume_status.clone(),
                    protection: drive.protection_status.clone(),
                    percentage: drive.encryption_percentage,
                });
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceState {
    spki_hash: SpkiHash,
    pub(crate) report: Option<Report>,
    pub(crate) meta_info: Option<MetaInfo>,
}

impl DeviceState {
    pub fn report(&self) -> Option<&Report> {
        self.report.as_ref()
    }

    pub fn meta_info(&self) -> Option<&MetaInfo> {
        self.meta_info.as_ref()
    }

    pub fn name(&self) -> Cow<'_, str> {
        if let Some(meta) = self.meta_info() {
            if !meta.name.is_empty() {
                return meta.name.as_str().into();
            }
        }
        if let Some(report) = self.report() {
            if let Some(hostname) = &report.system_report.hostname {
                return hostname.into();
            }
        }
        self.spki_hash.to_string().into()
    }

    pub fn os(&self) -> OSFamily {
        self.report()
            .map(|report| report.system_report.os_family)
            .unwrap_or(OSFamily::Unknown)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub current_version_identifier: String,
    pub system_report: SystemReport,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetaInfo {
    pub updated_at: u64,
    pub name: String,
    pub group: String,
    pub notes: String,
}
