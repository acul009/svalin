use std::time::Duration;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use rand::Rng;
use svalin_pki::{
    CreateCredentialsError, Credential, DeriveKeyError, KeyPair, SignatureVerificationError,
    UnverifiedCertificate, UseAsRootError, get_current_timestamp,
};
use svalin_rpc::{
    rpc::{
        command::{
            dispatcher::{DispatcherError, TakeableCommandDispatcher},
            handler::TakeableCommandHandler,
        },
        peer::Peer,
        session::{Session, SessionReadError, SessionWriteError},
    },
    transport::{
        combined_transport::CombinedTransport,
        tls_transport::{TlsClientError, TlsServerError, TlsTransport},
    },
    verifiers::skip_verify::SkipClientVerification,
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use super::{AgentInitPayload, ServerJoinManager};

pub struct JoinRequestHandler {
    manager: ServerJoinManager,
}

impl JoinRequestHandler {
    pub(super) fn new(manager: ServerJoinManager) -> Self {
        Self { manager }
    }
}

fn create_join_code() -> String {
    rand::rng().random_range(0..999999).to_string()
}

#[async_trait]
impl TakeableCommandHandler for JoinRequestHandler {
    type Request = ();

    fn key() -> String {
        "join_request".to_string()
    }

    async fn handle(
        &self,
        session: &mut Option<Session>,
        _: Self::Request,
        _: CancellationToken,
    ) -> Result<()> {
        if let Some(mut session) = session.take() {
            let mut join_code = create_join_code();
            while let Err(sess) = self.manager.add_session(join_code, session).await {
                session = sess;
                tokio::time::sleep(Duration::from_secs(5)).await;

                join_code = create_join_code();

                // todo: dont loop forever
            }

            Ok(())
        } else {
            Err(anyhow!("tried executing commandhandler with None"))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RequestJoinError {
    #[error("error reading join code: {0}")]
    JoinCodeReadError(#[source] SessionReadError),
    #[error("error sending join code through channel")]
    ChannelSendError,
    #[error("error reading join code from client: {0}")]
    SecondJoinCodeReadError(#[source] SessionReadError),
    #[error("error sending confirm status: {0}")]
    SendConfirmStatusError(#[source] SessionWriteError),
    #[error("confirm code did not match")]
    ConfirmError,
    #[error("error creating TLS client: {0}")]
    TlsCreateClientError(#[source] TlsClientError),
    #[error("error creating temp credentials: {0}")]
    CreateCredentialsError(#[source] CreateCredentialsError),
    #[error("error creating TLS server: {0}")]
    TlsCreateServerError(#[source] TlsServerError),
    #[error("error reading params: {0}")]
    ReadParamsError(#[source] SessionReadError),
    #[error("error deriving confirm key: {0}")]
    DeriveKeyError(#[source] DeriveKeyError),
    #[error("error sending request: {0}")]
    SendRequestError(#[source] SessionWriteError),
    #[error("error reading certificate: {0}")]
    ReadCertificateError(#[source] SessionReadError),
    #[error("error upgrading credentials: {0}")]
    UpgradeError(#[source] CreateCredentialsError),
    #[error("error reading upstream cert: {0}")]
    ReadUpstreamCertError(#[source] SessionReadError),
    #[error("error reading root cert: {0}")]
    ReadRootCertError(#[source] SessionReadError),
    #[error("error verifying root cert")]
    VerifyRootCertError,
    #[error("error using root cert: {0}")]
    UseRootError(#[source] UseAsRootError),
    #[error("error verifying upstream cert: {0}")]
    VerifyUpstreamCertError(#[source] SignatureVerificationError),
}

pub struct RequestJoin {
    pub address: String,
    pub join_code_channel: oneshot::Sender<String>,
    pub confirm_code_channel: oneshot::Sender<String>,
}

impl TakeableCommandDispatcher for RequestJoin {
    type Output = AgentInitPayload;

    type InnerError = RequestJoinError;

    type Request = ();

    fn key() -> String {
        JoinRequestHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
    ) -> Result<Self::Output, DispatcherError<RequestJoinError>> {
        if let Some(mut session) = session.take() {
            let join_code: String = session
                .read_object()
                .await
                .map_err(RequestJoinError::JoinCodeReadError)?;

            debug!("received join code from server: {join_code}");

            self.join_code_channel
                .send(join_code.clone())
                .map_err(|_| RequestJoinError::ChannelSendError)?;

            debug!("waiting for client to confirm join code");

            let join_code_confirm: String = session
                .read_object()
                .await
                .map_err(RequestJoinError::SecondJoinCodeReadError)?;

            debug!("received join code from client: {join_code_confirm}");

            if join_code != join_code_confirm {
                debug!("join codes do not match!");
                let answer: Result<(), ()> = Err(());
                session
                    .write_object(&answer)
                    .await
                    .map_err(RequestJoinError::SendConfirmStatusError)?;
                return Err(DispatcherError::Other(RequestJoinError::ConfirmError));
            } else {
                debug!("join codes match!");
                let answer: Result<(), ()> = Ok(());
                session
                    .write_object(&answer)
                    .await
                    .map_err(RequestJoinError::SendConfirmStatusError)?;
            }

            debug!("trying to establish tls connection");

            let (read, write, _) = session.destructure_transport();

            let temp_credentials =
                Credential::generate_root().map_err(RequestJoinError::CreateCredentialsError)?;

            let tls_transport = TlsTransport::server(
                CombinedTransport::new(read, write),
                SkipClientVerification::new(),
                &temp_credentials,
            )
            .await
            .map_err(RequestJoinError::TlsCreateServerError)?;

            let mut key_material = [0u8; 32];
            tls_transport
                .derive_key(&mut key_material, b"join_confirm_key", join_code.as_bytes())
                .unwrap();

            let (read, write) = tokio::io::split(tls_transport);

            let mut session = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

            debug!("server tls connection established");

            let params = session
                .read_object()
                .await
                .map_err(RequestJoinError::ReadParamsError)?;

            let confirm_code = super::derive_confirm_code(params, &key_material)
                .await
                .map_err(RequestJoinError::DeriveKeyError)?;

            debug!("generated confirm code: {confirm_code}");

            self.confirm_code_channel.send(confirm_code).unwrap();

            let keypair = KeyPair::generate();
            let public_key = keypair.export_public_key();
            debug!("sending request: {:?}", public_key);
            session
                .write_object(&public_key)
                .await
                .map_err(RequestJoinError::SendRequestError)?;

            let my_cert: UnverifiedCertificate = session
                .read_object()
                .await
                .map_err(RequestJoinError::ReadCertificateError)?;
            let my_credentials = keypair
                .upgrade(my_cert)
                .map_err(RequestJoinError::UpgradeError)?;

            let root: UnverifiedCertificate = session
                .read_object()
                .await
                .map_err(RequestJoinError::ReadRootCertError)?;
            let root = root.use_as_root().map_err(RequestJoinError::UseRootError)?;
            let upstream: UnverifiedCertificate = session
                .read_object()
                .await
                .map_err(RequestJoinError::ReadUpstreamCertError)?;
            let upstream = upstream
                .verify_signature(&root, get_current_timestamp())
                .map_err(RequestJoinError::VerifyUpstreamCertError)?;

            debug!("received all neccessary data to initialize agent");

            Ok(AgentInitPayload {
                credentials: my_credentials,
                address: self.address,
                root,
                upstream,
            })
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}
