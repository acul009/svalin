use std::collections::HashMap;

use svalin_pki::{
    Certificate, Credential, Fingerprint,
    mls::{KeyPackageIn, MlsClient, SvalinProvider},
};
use svalin_sysctl::sytem_report::SystemReport;

pub struct ClientAsyncCom {
    mls_client: MlsClient,
    credential: Credential,
    root: Certificate,
    device_status: HashMap<Certificate, SystemReport>,
}

impl ClientAsyncCom {
    pub fn create_device_group(&self, certificate: &Certificate) -> anyhow::Result<()> {
        let root_package = self.get_key_package(&self.root)?;
        let device_package = self.get_key_package(certificate)?;
        let mut group = self.mls_client.create_group()?;
        let (message, invitation) = group.add_members([root_package, device_package])?;
        todo!()
    }

    pub fn get_device_status(&self, certificate: &Certificate) -> anyhow::Result<&SystemReport> {
        todo!()
    }

    pub fn get_device_accesors(
        &self,
        certificate: &Certificate,
    ) -> anyhow::Result<Vec<Certificate>> {
        todo!()
    }

    fn get_key_package(&self, certificate: &Certificate) -> anyhow::Result<KeyPackageIn> {
        todo!()
    }
}
