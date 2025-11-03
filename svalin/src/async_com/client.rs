use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use svalin_pki::{
    Certificate, Credential,
    mls::{MlsClient, NewMember, message_types::Invitation},
};
use svalin_sysctl::sytem_report::SystemReport;

pub struct ClientAsyncCom {
    mls_client: MlsClient,
    credential: Credential,
    root: Certificate,
    device_status: Mutex<HashMap<Certificate, Arc<SystemReport>>>,
}

impl ClientAsyncCom {
    pub fn create_device_group(&self, certificate: &Certificate) -> anyhow::Result<Invitation> {
        let mut packages = Vec::with_capacity(2);
        packages.push(self.get_key_package(certificate)?);
        if self.credential.get_certificate() == &self.root {
            packages.push(self.get_key_package(&self.root)?);
        }
        let mut group = self.mls_client.create_group()?;
        let (_first_message, invitation) = group.add_members(packages)?;
        Ok(invitation)
    }

    pub fn get_device_status(
        &self,
        certificate: &Certificate,
    ) -> anyhow::Result<Arc<SystemReport>> {
        self.device_status
            .lock()
            .unwrap()
            .get(certificate)
            .cloned()
            .ok_or(anyhow!("missing device status"))
    }

    pub fn get_device_accesors(
        &self,
        certificate: &Certificate,
    ) -> anyhow::Result<Vec<Certificate>> {
        todo!()
    }

    fn get_key_package(&self, certificate: &Certificate) -> anyhow::Result<NewMember> {
        let key_package = todo!();
        Ok(NewMember {
            key_package,
            member: certificate.clone(),
        })
    }
}
