use svalin_pki::mls::{agent::EncodedReport, client::MlsClient};
use svalin_rpc::rpc::command::dispatcher::CommandDispatcher;
use svalin_sysctl::sytem_report::SystemReport;

pub struct UpdateSystemReport(pub EncodedReport<SystemReport>);

impl CommandDispatcher for UpdateSystemReport {
    type Output = ();

    type Error = anyhow::Error;

    type Request = EncodedReport<SystemReport>;

    fn key() -> String {
        "update-system-report".to_string()
    }

    fn get_request(&self) -> &Self::Request {
        &self.0
    }

    async fn dispatch(
        self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        todo!()
    }
}
