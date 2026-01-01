use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use svalin_pki::{Certificate, Credential, mls::MlsClient};
use svalin_sysctl::sytem_report::SystemReport;

use crate::client::Client;

pub struct ClientAsyncCom {
    client: Arc<Client>,
    mls_client: MlsClient,
    credential: Credential,
    root: Certificate,
    device_status: Mutex<HashMap<Certificate, Arc<SystemReport>>>,
}

impl ClientAsyncCom {
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

    async fn get_init_device_accessors(&self) -> anyhow::Result<Vec<UnvalidatedNewMember>> {
        {
            let certs = self
                .client
                .get_user_certificates(self.credential.get_certificate().issuer())
                .await?;

            let add_root = &certs.cert_chain != &self.root;

            let mut certs = certs.to_vec();
            if add_root {
                let root_certs = self
                    .client
                    .get_user_certificates(self.root.spki_hash())
                    .await?;
                certs.push(self.root.clone());
                certs.extend_from_slice(&root_certs.session_certs);
            }

            let certs = certs
                .iter()
                .filter(|cert| *cert != self.credential.get_certificate())
                .map(|cert| cert.spki_hash().clone())
                .collect();

            let mut packages = Vec::new();

            let key_packages = self.client.get_key_packages(certs)?;

            todo!()

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

    fn get_key_package(&self, certificate: &Certificate) -> anyhow::Result<UnvalidatedNewMember> {
        let key_package = todo!();
        Ok(UnvalidatedNewMember {
            key_package,
            member: certificate.clone(),
        })
    }
}
