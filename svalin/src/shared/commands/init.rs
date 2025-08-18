use anyhow::Result;
use svalin_pki::{
    Certificate, CreateCertificateError, CreateCredentialsError, Credential, ExportedPublicKey,
    KeyPair,
};

use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError, SessionWriteError},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;

pub(crate) struct InitHandler {
    channel: mpsc::Sender<(Certificate, Credential)>,
}

impl InitHandler {
    pub fn new(conf: mpsc::Sender<(Certificate, Credential)>) -> Self {
        Self { channel: conf }
    }
}

#[async_trait]
impl CommandHandler for InitHandler {
    type Request = Certificate;

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        _: CancellationToken,
    ) -> anyhow::Result<()> {
        debug!("incoming init request");

        if self.channel.is_closed() {
            return Ok(());
        }

        let root = request;

        let keypair = KeyPair::generate();
        let public_key = keypair.export_public_key();
        session.write_object(&public_key).await?;

        let my_cert: Certificate = session.read_object().await?;
        let my_credentials = keypair.upgrade(my_cert)?;

        debug!("init request handled");

        session
            .write_object::<std::result::Result<(), ()>>(&Ok(()))
            .await?;

        let _: Result<(), SessionReadError> = session.read_object().await;

        self.channel.send((root, my_credentials)).await?;

        Ok(())
    }

    fn key() -> String {
        "init".to_owned()
    }
}

pub struct Init {
    root: Credential,
}

impl Init {
    pub fn new() -> Result<Self, CreateCredentialsError> {
        let root = Credential::generate_root()?;

        Ok(Self { root })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("error reading request: {0}")]
    ReadRequestError(SessionReadError),
    #[error("error creating certificate for public key: {0}")]
    CreateCertError(CreateCertificateError),
    #[error("error writing server certificate: {0}")]
    WriteServerCertError(SessionWriteError),
    #[error("error reading success: {0}")]
    ReadSuccessError(SessionReadError),
    #[error("error writing confirm: {0}")]
    WriteConfirmError(SessionWriteError),
}

impl CommandDispatcher for Init {
    type Output = (Credential, Certificate);
    type Request = Certificate;
    type Error = InitError;

    fn key() -> String {
        InitHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        self.root.get_certificate()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        debug!("sending init request");

        let public_key: ExportedPublicKey = session
            .read_object()
            .await
            .map_err(InitError::ReadRequestError)?;
        let server_cert: Certificate = self
            .root
            .create_server_certificate_for_key(&public_key)
            .map_err(InitError::CreateCertError)?;

        session
            .write_object(&server_cert)
            .await
            .map_err(InitError::WriteServerCertError)?;

        let _ok: std::result::Result<(), ()> = session
            .read_object()
            .await
            .map_err(InitError::ReadSuccessError)?;

        session
            .write_object(&())
            .await
            .map_err(InitError::WriteConfirmError)?;

        debug!("init completed");

        Ok((self.root, server_cert))
    }
}
