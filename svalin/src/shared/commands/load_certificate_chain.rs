use async_trait::async_trait;
use svalin_pki::{CertificateChain, Fingerprint};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use tokio_util::sync::CancellationToken;

pub struct LoadCertificateChainHandler {}

#[async_trait]
impl CommandHandler for LoadCertificateChainHandler {
    type Request = Fingerprint;

    fn key() -> String {
        "load_certificate_chain".to_string()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        todo!()
    }
}

struct LoadCertificateChain {
    fingerprint: Fingerprint,
}

impl LoadCertificateChain {
    pub fn new(fingerprint: Fingerprint) -> Self {
        Self { fingerprint }
    }
}

pub enum LoadCertificateChainError {}

impl CommandDispatcher for LoadCertificateChain {
    type Output = CertificateChain;

    type Error = LoadCertificateChainError;

    type Request = <LoadCertificateChainHandler as CommandHandler>::Request;

    fn key() -> String {
        todo!()
    }

    fn get_request(&self) -> &Self::Request {
        todo!()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        todo!()
    }
}
