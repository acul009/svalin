use anyhow::{anyhow, Ok, Result};
use svalin_macros::rpc_dispatch;
use svalin_pki::{Certificate, CertificateRequest, Keypair, PermCredentials};
use svalin_rpc::{CommandHandler, Session, SessionOpen};

use async_trait::async_trait;
use tokio::sync::mpsc;

pub(crate) struct InitHandler {
    channel: mpsc::Sender<(Certificate, PermCredentials)>,
}

impl InitHandler {
    pub fn new(conf: mpsc::Sender<(Certificate, PermCredentials)>) -> Self {
        Self { channel: conf }
    }
}

fn init_key() -> String {
    "init".to_owned()
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

        println!("init request handled");

        self.channel.send((root, my_credentials)).await?;

        Ok(())
    }

    fn key(&self) -> String {
        init_key()
    }
}

#[rpc_dispatch(init_key())]
pub(crate) async fn init(session: &mut Session<SessionOpen>) -> Result<PermCredentials> {
    println!("sending init request");
    let root = Keypair::generate()?.to_self_signed_cert()?;
    session.write_object(root.get_certificate()).await?;

    let raw_request: String = session.read_object().await?;
    let request = CertificateRequest::from_string(raw_request)?;
    let server_cert: Certificate = root.approve_request(request)?;

    session.write_object(&server_cert).await?;

    println!("init completed");

    Ok(root)
}