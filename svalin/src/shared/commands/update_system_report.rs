use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use svalin_pki::mls::{agent::EncodedReport, client::MlsClient, delivery_service::DeliveryService};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    peer::Peer,
    session::Session,
};
use svalin_sysctl::sytem_report::SystemReport;
use tokio_util::sync::CancellationToken;

use crate::server::message_store::MessageStore;

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

pub struct UpdateSystemReportHandler {
    mls: Arc<DeliveryService>,
    message_store: Arc<MessageStore>,
}

#[async_trait]
impl CommandHandler for UpdateSystemReportHandler {
    type Request = EncodedReport<SystemReport>;

    fn key() -> String {
        UpdateSystemReport::key()
    }
    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let Peer::Certificate(peer) = session.peer() else {
            return Err(anyhow!("unexpected anonymous peer"));
        };
        let report = request.raw();
        self.mls
            .process_device_group_message(peer.spki_hash(), &report)
            .await?;
        todo!()
    }
}
