use anyhow::{Result, anyhow};
use async_trait::async_trait;
use svalin_pki::{
    ArgonParams, Certificate, CreateCertificateError, Credential, DeriveKeyError, ExactVerififier,
    ExportedPublicKey, RootCertificate,
    mls::{
        OpenMlsProvider,
        client::MlsClient,
        key_package::{KeyPackageError, UnverifiedKeyPackage},
    },
};
use svalin_rpc::{
    rpc::{
        command::{
            dispatcher::{DispatcherError, TakeableCommandDispatcher},
            handler::CommandHandler,
        },
        peer::Peer,
        session::{Session, SessionReadError, SessionWriteError},
    },
    transport::{
        combined_transport::CombinedTransport,
        tls_transport::{TlsClientError, TlsTransport},
    },
    verifiers::skip_verify::SkipServerVerification,
};
use tokio::{io::copy_bidirectional, select, sync::oneshot};
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

use crate::client::Client;

use super::ServerJoinManager;

pub struct JoinAcceptHandler {
    manager: ServerJoinManager,
}

impl JoinAcceptHandler {
    pub(super) fn new(manager: ServerJoinManager) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl CommandHandler for JoinAcceptHandler {
    type Request = ();

    fn key() -> String {
        "accept_join".to_string()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let join_code: String = session.read_object().await?;

        let agent_session = self.manager.get_session(&join_code);

        match agent_session {
            Some(mut agent_session) => {
                let answer: Result<(), ()> = Ok(());
                session.write_object(&answer).await?;

                debug!("forwarding session to agent");

                let (read1, write1) = session.borrow_transport();
                let (read2, write2) = agent_session.borrow_transport();

                let mut transport1 = CombinedTransport::new(read1, write1);
                let mut transport2 = CombinedTransport::new(read2, write2);

                select! {
                        _ = cancel.cancelled() => {}
                        result = copy_bidirectional(&mut transport1, &mut transport2) => {result?;}
                }

                debug!("finished forwarding join accept session");

                Ok(())
            }
            None => {
                let answer: Result<(), ()> = Err(());
                session.write_object(&answer).await?;

                Ok(())
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AcceptJoinError {
    #[error("Session read error: {0}")]
    SessionReadError(#[from] SessionReadError),
    #[error("Session write error: {0}")]
    SessionWriteError(#[from] SessionWriteError),
    #[error("Agent not found")]
    AgentNotFound,
    #[error("Agent did not aknowledge connection")]
    NotAknowledged,
    #[error("Recv error: {0}")]
    RecvError(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("TLS client error: {0}")]
    TlsClientError(#[from] TlsClientError),
    #[error("Derive key error: {0}")]
    DeriveKeyError(#[from] DeriveKeyError),
    #[error("Confirm code mismatch")]
    ConfirmCodeMismatch,
    #[error("error creating agent certificate: {0}")]
    CreateCertificateError(#[from] CreateCertificateError),
    #[error("error verifying key package: {0}")]
    KeyPackageError(#[from] KeyPackageError),
}

pub struct AcceptJoin<'a> {
    pub join_code: String,
    pub confirm_code: oneshot::Sender<oneshot::Sender<String>>,
    pub client: &'a Client,
}

impl<'a> TakeableCommandDispatcher for AcceptJoin<'a> {
    type Output = Certificate;
    type InnerError = AcceptJoinError;

    type Request = ();

    fn key() -> String {
        JoinAcceptHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
    ) -> Result<Self::Output, DispatcherError<Self::InnerError>> {
        if let Some(session) = session.take() {
            {
                let confirm_code_result =
                    prepare_agent_enroll(session, &self.join_code, self.client.user_credential()).await;

                match confirm_code_result {
                    Err(err) => {

                        Err(err)
                    }
                    Ok((confirm_code, session_e2e)) => {
                        handle_agent_enroll(
                            self.waiting_for_confirm,
                            self.confirm_code_channel,
                            self.credentials,
                            self.root,
                            self.upstream,
                            session_e2e,
                            confirm_code,
                            self.mls,
                        )
                        .await
                    }
                }
            }
            .map_err(DispatcherError::Other)
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}

async fn handle_agent_enroll(
    waiting_for_confirm: oneshot::Sender<Result<(), anyhow::Error>>,
    confirm_code_channel: oneshot::Receiver<String>,
    credentials: &Credential,
    root: &RootCertificate,
    upstream: &Certificate,
    mut session_e2e: Session,
    confirm_code: String,
    mls: &MlsClient,
) -> Result<Certificate, AcceptJoinError> {
    waiting_for_confirm.send(Ok(())).unwrap();

    let remote_confirm_code = confirm_code_channel.await?;

    debug!("received confirm code from user: {remote_confirm_code}");

    if confirm_code != remote_confirm_code {
        return Err(AcceptJoinError::ConfirmCodeMismatch);
    }

    debug!("Confirm Codes match!");

    let public_key: ExportedPublicKey = session_e2e.read_object().await?;
    debug!("received public key: {:?}", public_key);
    let agent_cert = credentials.create_agent_certificate_for_key(&public_key)?;

    session_e2e.write_object(agent_cert.as_unverified()).await?;
    session_e2e.write_object(root.as_unverified()).await?;
    session_e2e.write_object(upstream.as_unverified()).await?;

    let key_package: UnverifiedKeyPackage = session_e2e.read_object().await?;

    let verifier = ExactVerififier::new(agent_cert.clone());

    let key_package = key_package
        .verify(mls.provider().crypto(), mls.protocol_version(), &verifier)
        .await?;

    mls.create_device_group(key_package)

    compile_error!("Continue here!");

    Ok(agent_cert)
}

#[instrument(skip_all)]
async fn prepare_agent_enroll(
    mut session: Session,
    join_code: &String,
    credentials: &Credential,
) -> Result<(String, Session), AcceptJoinError> {
    session.write_object(join_code).await?;

    let found: std::result::Result<(), ()> = session.read_object().await?;

    if let Err(()) = found {
        return Err(AcceptJoinError::AgentNotFound);
    }

    debug!("connected to agent, sending join code for confirmation");

    // confirm join code with agent
    session.write_object(join_code).await?;

    debug!("waiting for agent to confirm join code");

    let ready: std::result::Result<(), ()> = session.read_object().await?;

    if let Err(()) = ready {
        return Err(AcceptJoinError::NotAknowledged);
    }

    debug!("agent confirmed join code");

    // establish tls session

    debug!("trying to establish tls connection");

    let (read, write, _) = session.destructure_transport();

    let tls_transport = TlsTransport::client(
        CombinedTransport::new(read, write),
        SkipServerVerification::new(),
        credentials,
    )
    .await?;

    let mut key_material = [0u8; 32];
    let key_material = tls_transport
        .derive_key(&mut key_material, b"join_confirm_key", join_code.as_bytes())
        .unwrap();

    let (read, write) = tokio::io::split(tls_transport);

    let mut session = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

    debug!("client tls connection established");

    let params = ArgonParams::basic();

    session.write_object(&params).await?;

    let confirm_code = super::derive_confirm_code(params, &key_material).await?;

    debug!("client side confirm code: {confirm_code}");

    Ok((confirm_code, session))
}
