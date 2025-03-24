use anyhow::Result;
use svalin_pki::{
    ApproveRequestError, Certificate, CertificateRequest, CertificateRequestParseError, Keypair,
    PermCredentials, ToSelfSingedError,
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

        let keypair = Keypair::generate();
        let request = keypair.generate_request()?;
        session.write_object(&request).await?;

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
    root: PermCredentials,
}

impl Init {
    pub fn new() -> Result<Self, ToSelfSingedError> {
        let root = Keypair::generate().to_self_signed_cert()?;

        Ok(Self { root })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("error reading request: {0}")]
    ReadRequestError(SessionReadError),
    #[error("error parsing request: {0}")]
    RequestParseError(CertificateRequestParseError),
    #[error("error approving certificate request: {0}")]
    ApproveRequestError(ApproveRequestError),
    #[error("error writing server certificate: {0}")]
    WriteServerCertError(SessionWriteError),
    #[error("error reading success: {0}")]
    ReadSuccessError(SessionReadError),
    #[error("error writing confirm: {0}")]
    WriteConfirmError(SessionWriteError),
}

#[async_trait]
impl CommandDispatcher for Init {
    type Output = (PermCredentials, Certificate);
    type Request = Certificate;
    type Error = InitError;

    fn key() -> String {
        InitHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        self.root.get_certificate().clone()
    }

    async fn dispatch(
        self,
        session: &mut Session,
        _: Self::Request,
    ) -> Result<Self::Output, Self::Error> {
        debug!("sending init request");

        let raw_request: String = session
            .read_object()
            .await
            .map_err(InitError::ReadRequestError)?;
        let request =
            CertificateRequest::from_string(raw_request).map_err(InitError::RequestParseError)?;
        let server_cert: Certificate = self
            .root
            .approve_request(request)
            .map_err(InitError::ApproveRequestError)?;

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
