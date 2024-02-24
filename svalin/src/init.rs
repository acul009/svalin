use anyhow::Result;
use svalin_macros::rpc_dispatch;
use svalin_pki::{Certificate, CertificateRequest, Keypair};
use svalin_rpc::{CommandHandler, Session, SessionOpen};

use async_trait::async_trait;

pub(crate) struct InitHandler {}

#[async_trait]
impl CommandHandler for InitHandler {
    async fn handle(&self, mut session: Session<SessionOpen>) -> anyhow::Result<()> {
        let root: Certificate = session.read_object().await?;

        let keypair = Keypair::generate()?;
        let request = keypair.generate_request()?;
        session.write_object(&request).await?;

        todo!()
    }

    fn key(&self) -> String {
        "init".into()
    }
}

#[rpc_dispatch(init_key)]
async fn init(session: &mut Session<SessionOpen>, initname: String) -> Result<String> {
    let raw_request: String = session.read_object().await?;
    let request = CertificateRequest::from_string(raw_request);

    todo!()
}
