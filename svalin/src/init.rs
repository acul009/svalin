use anyhow::{anyhow, Ok, Result};
use svalin_macros::rpc_dispatch;
use svalin_pki::{Certificate, CertificateRequest, Keypair, PermCredentials};
use svalin_rpc::{CommandHandler, Session, SessionOpen};

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot, Mutex};

pub(crate) struct InitHandler {
    channel: mpsc::Sender<(Certificate, PermCredentials)>,
}

impl InitHandler {
    pub fn new(conf: mpsc::Sender<(Certificate, PermCredentials)>) -> Self {
        Self { channel: conf }
    }
}

#[async_trait]
impl CommandHandler for InitHandler {
    async fn handle(&self, mut session: Session<SessionOpen>) -> anyhow::Result<()> {
        println!("incoming init request");

        if self.channel.is_closed() {
            return Ok(());
        }

        let root: Certificate = session.read_object().await?;

        let keypair = Keypair::generate()?;
        let request = keypair.generate_request()?;
        session.write_object(&request).await?;

        let my_cert: Certificate = session.read_object().await?;
        let my_credentials = keypair.upgrade(my_cert)?;

        self.channel.send((root, my_credentials)).await?;

        Ok(())
    }

    fn key(&self) -> String {
        "init".into()
    }
}

#[rpc_dispatch(init_key)]
pub(crate) async fn init(session: &mut Session<SessionOpen>, initname: String) -> Result<String> {
    println!("sending init request");
    let root = Keypair::generate()?.to_self_signed_cert()?;
    session.write_object(root.get_certificate()).await?;

    let raw_request: String = session.read_object().await?;
    let request = CertificateRequest::from_string(raw_request)?;
    let server_cert: Certificate = root.approve_request(request)?;

    session.write_object(&server_cert).await?;

    todo!()
}
