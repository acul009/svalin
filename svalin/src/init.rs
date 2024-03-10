use anyhow::{anyhow, Ok, Result};
use svalin_macros::rpc_dispatch;
use svalin_pki::{Certificate, CertificateRequest, Keypair, PermCredentials};
use svalin_rpc::{CommandHandler, Session, SessionOpen};

use async_trait::async_trait;
use tokio::sync::{oneshot, Mutex};

pub(crate) struct InitHandler {
    conf: Mutex<oneshot::Sender<(Certificate, PermCredentials)>>,
}

impl InitHandler {
    pub fn new(conf: oneshot::Sender<(Certificate, PermCredentials)>) -> Self {
        todo!()
    }
}

#[async_trait]
impl CommandHandler for InitHandler {
    async fn handle(&self, mut session: Session<SessionOpen>) -> anyhow::Result<()> {
        let root: Certificate = session.read_object().await?;

        let keypair = Keypair::generate()?;
        let request = keypair.generate_request()?;
        session.write_object(&request).await?;

        let my_cert: Certificate = session.read_object().await?;
        let my_credentials = keypair.upgrade(my_cert)?;

        self.conf
            .send((root, my_credentials))
            .map_err(|err| anyhow!("Could not send init config"));

        Ok(())
    }

    fn key(&self) -> String {
        "init".into()
    }
}

#[rpc_dispatch(init_key)]
async fn init(session: &mut Session<SessionOpen>, initname: String) -> Result<String> {
    let root = Keypair::generate()?.to_self_signed_cert()?;
    session.write_object(root.get_certificate()).await?;

    let raw_request: String = session.read_object().await?;
    let request = CertificateRequest::from_string(raw_request)?;
    let server_cert = root.approve_request(request)?;

    session.write_object(&server_cert).await?;

    todo!()
}
