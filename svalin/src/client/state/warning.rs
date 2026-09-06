use svalin_pki::SpkiHash;

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

    pub fn update_device(&mut self, device: &SpkiHash, warnings: Vec<Device>) {
        self.warnings = std::mem::take(&mut self.warnings)
            .into_iter()
            .filter(|warning| match warning {
                Warning::Device(spki, _) => spki != device,
                #[allow(unreachable_patterns)]
                _ => true,
            })
            .chain(
                warnings
                    .into_iter()
                    .map(|warning| Warning::Device(device.clone(), warning)),
            )
            .collect();
        self.warnings.sort_by_key(|warning| warning.severity());
    }
}

#[derive(Clone, Debug)]
pub enum Warning {
    Device(SpkiHash, Device),
}

impl Warning {
    pub fn severity(&self) -> Severity {
        match self {
            Self::Device(_, warning) => warning.severity(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    High,
    Normal,
    Low,
}

#[derive(Clone, Debug)]
pub enum Device {
    DeviceGroupBroken(String),
    DiskSpaceLow { disk: String, free: u64, total: u64 },
    MissingName,
}

impl Device {
    pub fn severity(&self) -> Severity {
        match self {
            Self::DeviceGroupBroken(_) => Severity::High,
            Self::DiskSpaceLow { .. } => Severity::Normal,
            Self::MissingName => Severity::Low,
        }
    }
}
