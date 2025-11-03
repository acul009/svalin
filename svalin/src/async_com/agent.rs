use svalin_pki::{Certificate, Credential, mls::SvalinProvider};
use svalin_sysctl::sytem_report::SystemReport;

pub struct AgentAsyncCom {
    mls_provider: SvalinProvider,
    credential: Credential,
}

impl AgentAsyncCom {
    pub fn upload_device_status(&self, report: SystemReport) -> anyhow::Result<()> {
        todo!()
    }

    pub fn get_device_accesors(
        &self,
        certificate: &Certificate,
    ) -> anyhow::Result<Vec<Certificate>> {
        todo!()
    }
}
