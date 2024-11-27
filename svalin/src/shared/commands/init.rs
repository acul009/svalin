use anyhow::Result;
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

#[async_trait]
impl CommandHandler for InitHandler {
    type Request = Certificate;

    async fn handle(&self, session: &mut Session, request: Self::Request) -> anyhow::Result<()> {
        debug!("incoming init request");

        if self.channel.is_closed() {
            return Ok(());
        }

        let root = request;

        let keypair = Keypair::generate()?;
        let request = keypair.generate_request()?;
        session.write_object(&request).await?;

        let my_cert: Certificate = session.read_object().await?;
        let my_credentials = keypair.upgrade(my_cert)?;

        debug!("init request handled");

        session
            .write_object::<std::result::Result<(), ()>>(&Ok(()))
            .await?;

        let _: Result<()> = session.read_object().await;

        self.channel.send((root, my_credentials)).await?;

        Ok(())
    }

    fn key() -> String {
        "init".to_owned()
    }
}

pub struct Init {
    root: PermCredentials,
}

impl Init {
    pub fn new() -> Result<Self> {
        let root = Keypair::generate()?.to_self_signed_cert()?;

        Ok(Self { root })
    }
}

#[async_trait]
impl CommandDispatcher for Init {
    type Output = (PermCredentials, Certificate);
    type Request = Certificate;

    fn key() -> String {
        InitHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        self.root.get_certificate().clone()
    }

    async fn dispatch(self, session: &mut Session, _: Self::Request) -> Result<Self::Output> {
        debug!("sending init request");

        let raw_request: String = session.read_object().await?;
        let request = CertificateRequest::from_string(raw_request)?;
        let server_cert: Certificate = self.root.approve_request(request)?;

        session.write_object(&server_cert).await?;

        let _ok: std::result::Result<(), ()> = session.read_object().await?;

        session.write_object(&()).await?;

        debug!("init completed");

        Ok((self.root, server_cert))
    }
}
