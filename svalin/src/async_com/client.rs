use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use svalin_pki::{
    Certificate, CertificateType, Credential,
    mls::{MlsClient, NewMember, message_types::Invitation},
};
use svalin_sysctl::sytem_report::SystemReport;

use crate::client::Client;

pub struct ClientAsyncCom {
    client: Client,
    mls_client: MlsClient,
    credential: Credential,
    root: Certificate,
    device_status: Mutex<HashMap<Certificate, Arc<SystemReport>>>,
}

impl ClientAsyncCom {
    pub async fn create_device_group(&self, device: &Certificate) -> anyhow::Result<Invitation> {
        let mut group = self.mls_client.create_group()?;
        let (_first_message, invitation) =
            group.add_members(self.get_init_device_accessors(user_certs)?)?;
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

    async fn get_init_device_accessors(&self) -> anyhow::Result<Vec<NewMember>> {
        {
            let certs = self
                .client
                .get_user_certificates(self.credential.get_certificate().issuer())
                .await?;

            let add_root = &certs.user_cert != &self.root;

            let mut certs = certs.to_vec();
            if add_root {
                certs.push(self.root.clone());
            }

            let certs = certs
                .iter()
                .filter(|cert| cert != self.credential.get_certificate());

            let mut packages = Vec::new();

            for certificate in certs {
                self.
            }

            // let certs = user_certs
            //     .session_certs
            //     .into_iter()
            //     .filter(|session| session != self.credential.get_certificate())
            //     .collect::<Vec<_>>();
            // certs.push(self.root);
            // // Self not required, as it already creates the group
            // [&self.root, user_certs.user_cert]
            //     .into_iter()
            //     .map(|certificate| self.get_key_package(certificate))
            //     .collect()
        }
    }

    fn get_key_package(&self, certificate: &Certificate) -> anyhow::Result<NewMember> {
        let key_package = todo!();
        Ok(NewMember {
            key_package,
            member: certificate.clone(),
        })
    }
}
