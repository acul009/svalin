use anyhow::Result;
use svalin_macros::rpc_dispatch;
use svalin_pki::{Certificate, CertificateRequest, Keypair, PermCredentials};

use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use tokio::sync::mpsc;
use tracing::debug;

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
    async fn handle(&self, session: &mut Session) -> anyhow::Result<()> {
        debug!("incoming init request");

        if self.channel.is_closed() {
            return Ok(());
        }

        let root: Certificate = session.read_object().await?;

        let keypair = Keypair::generate()?;
        let request = keypair.generate_request()?;
        session.write_object(&request).await?;

        let my_cert: Certificate = session.read_object().await?;
        let my_credentials = keypair.upgrade(my_cert)?;

        debug!("init request handled");

        session
            .write_object::<std::result::Result<(), ()>>(&Ok(()))
            .await?;

        self.channel.send((root, my_credentials)).await?;

        Ok(())
    }

    fn key(&self) -> String {
        init_key()
    }
}

pub struct Init;

#[async_trait]
impl CommandDispatcher<(PermCredentials, Certificate)> for Init {
    fn key(&self) -> String {
        init_key()
    }

    async fn dispatch(self, session: &mut Session) -> Result<(PermCredentials, Certificate)> {
        debug!("sending init request");
        let root = Keypair::generate()?.to_self_signed_cert()?;
        session.write_object(root.get_certificate()).await?;

        let raw_request: String = session.read_object().await?;
        let request = CertificateRequest::from_string(raw_request)?;
        let server_cert: Certificate = root.approve_request(request)?;

        session.write_object(&server_cert).await?;

        let _ok: std::result::Result<(), ()> = session.read_object().await?;

        debug!("init completed");

        Ok((root, server_cert))
    }
}
