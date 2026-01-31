use anyhow::{Result, anyhow};
use async_trait::async_trait;
use svalin_pki::{
    ArgonParams, Certificate, CreateCertificateError, Credential, DeriveKeyError, ExactVerififier,
    ExportedPublicKey, RootCertificate,
    mls::{
        OpenMlsProvider,
        client::{CreateDeviceGroupError, MlsClient},
        key_package::{KeyPackageError, UnverifiedKeyPackage},
    },
};
use svalin_rpc::{
    rpc::{
        command::{
            dispatcher::{DispatcherError, TakeableCommandDispatcher},
            handler::CommandHandler,
        },
        connection::ConnectionDispatchError,
        peer::Peer,
        session::{Session, SessionReadError, SessionWriteError},
    },
    transport::{
        aucpace_transport::{AucPaceClientError, AucPaceTransport},
        combined_transport::CombinedTransport,
        session_transport::SessionTransport,
        tls_transport::{TlsClientError, TlsTransport},
    },
    verifiers::skip_verify::SkipServerVerification,
};
use tokio::{io::copy_bidirectional, select, sync::oneshot};
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

use crate::{
    client::{Client, GetKeyPackagesError},
    shared::commands::get_key_packages::GetKeyPackagesDispatcherError,
};

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

                let transport1 = session.borrow_transport();
                let transport2 = agent_session.borrow_transport();

                select! {
                        _ = cancel.cancelled() => {}
                        result = copy_bidirectional(transport1,transport2) => {result?;}
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
    #[error("user seems to have aborted this join process")]
    Aborted,
    #[error("error establishing encrypted tunnel: {0}")]
    AucPaceError(#[from] AucPaceClientError),
    #[error("error getting additional key packages from server: {0}")]
    GetKeyPackagesError(#[from] GetKeyPackagesError),
    #[error("error creating device group: {0}")]
    CreateDeviceGroupError(#[from] CreateDeviceGroupError),
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
        if let Some(mut session) = session.take() {
            // Sending join code to server
            session
                .write_object(&self.join_code)
                .await
                .map_err(AcceptJoinError::SessionWriteError)?;

            // If the server sends back ok, it has found an agent with the given join code.
            // After this point the connection will be patched through to the agent,
            // so at this point we have a direct (but insecure) connection to the agent.
            let found: std::result::Result<(), ()> = session
                .read_object()
                .await
                .map_err(AcceptJoinError::SessionReadError)?;
            if let Err(()) = found {
                return Err(AcceptJoinError::AgentNotFound.into());
            }

            // At this point the agent generates a random confirm code and displays it.
            // Our application should inform the user that a client was reached successfully and
            // that they should now input the confirm code.
            // The user can obtain the code either themselves or get someone else to read it to them over the phone.
            // That also means the code only a few digits long.

            let (confirm_send, confirm_recv) = oneshot::channel();

            let result = self.confirm_code.send(confirm_send);
            if result.is_err() {
                return Err(AcceptJoinError::Aborted.into());
            }

            let confirm_code = confirm_recv.await.map_err(|_| AcceptJoinError::Aborted)?;

            // Now that we have the confirmation code, we can open an encrypted tunnel using
            // AucPace to ensure security even with this relatively insecure short code.

            let (transport, _) = session.destructure();
            let transport = AucPaceTransport::client(transport, confirm_code.into_bytes())
                .await
                .map_err(AcceptJoinError::AucPaceError)?;

            handle_agent_enroll(transport, self.client)
                .await
                .map_err(DispatcherError::Other)
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}

async fn handle_agent_enroll(
    transport: AucPaceTransport<Box<dyn SessionTransport>>,
    client: &Client,
) -> Result<Certificate, AcceptJoinError> {
    let mut session_e2e = Session::new(Box::new(transport), Peer::Anonymous);
    // At this point we have a secure channel to the agent.
    // The first order of business is creating a certificate for the agent
    // and providing the agent with the root and upstream certificates.

    let public_key: ExportedPublicKey = session_e2e.read_object().await?;
    debug!("received public key: {:?}", public_key);

    // Creating certificate for agent
    let agent_cert = client
        .user_credential()
        .create_agent_certificate_for_key(&public_key)?;

    // Sending all 3 required certificates to the agent
    session_e2e.write_object(agent_cert.as_unverified()).await?;
    session_e2e
        .write_object(client.root_certificate().as_unverified())
        .await?;
    session_e2e
        .write_object(client.upstream_certificate().as_unverified())
        .await?;

    // Now the agent has it's credentials to both connect to the server and work with MLS.
    // The next step here is to create an MLS group to both distribute system reports from the agent
    // as well as record who has access to the system.
    let mls = client.mls();

    let agent_key_package: UnverifiedKeyPackage = session_e2e.read_object().await?;

    let verifier = ExactVerififier::new(agent_cert.clone());

    let agent_key_package = agent_key_package
        .verify(mls.provider().crypto(), mls.protocol_version(), &verifier)
        .await?;

    let others = client
        .get_key_packages(&[
            client.user_credential().get_certificate().clone(),
            client.root_certificate().as_certificate().clone(),
        ])
        .await?;

    let group_info = mls.create_device_group(agent_key_package, others).await?;

    session_e2e.write_object(&group_info).await?;

    Ok(agent_cert)
}
