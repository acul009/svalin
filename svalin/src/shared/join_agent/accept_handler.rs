use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use svalin_pki::{ArgonParams, Certificate, CertificateRequest, PermCredentials};
use svalin_rpc::{
    rpc::{
        command::{dispatcher::TakeableCommandDispatcher, handler::CommandHandler},
        peer::Peer,
        session::Session,
    },
    transport::{combined_transport::CombinedTransport, tls_transport::TlsTransport},
    verifiers::skip_verify::SkipServerVerification,
};
use tokio::io::copy_bidirectional;
use tracing::{debug, instrument};

use super::ServerJoinManager;

pub struct JoinAcceptHandler {
    manager: ServerJoinManager,
}

fn accept_join_code() -> String {
    "accept_join".to_string()
}

impl JoinAcceptHandler {
    pub(super) fn new(manager: ServerJoinManager) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl CommandHandler for JoinAcceptHandler {
    fn key(&self) -> String {
        accept_join_code()
    }

    async fn handle(&self, session: &mut Session) -> anyhow::Result<()> {
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

                copy_bidirectional(&mut transport1, &mut transport2).await?;

                debug!("finished forwarding session");

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

pub struct AcceptJoin<'a> {
    pub join_code: String,
    pub waiting_for_confirm: tokio::sync::oneshot::Sender<Result<()>>,
    pub confirm_code_channel: tokio::sync::oneshot::Receiver<String>,
    pub credentials: &'a PermCredentials,
    pub root: &'a Certificate,
    pub upstream: &'a Certificate,
}

#[async_trait]
impl<'a> TakeableCommandDispatcher for AcceptJoin<'a> {
    type Output = Certificate;

    fn key(&self) -> String {
        accept_join_code()
    }

    async fn dispatch(self, session: &mut Option<Session>) -> Result<Self::Output> {
        if let Some(session) = session.take() {
            let confirm_code_result =
                prepare_agent_enroll(session, self.join_code, self.credentials)
                    .await
                    .context("error during enrollment preparation");

            match confirm_code_result {
                Err(err) => {
                    let err_copy = anyhow!("{}", err);
                    self.waiting_for_confirm.send(Err(err)).unwrap();

                    Err(err_copy)
                }
                Ok((confirm_code, mut session_e2e)) => {
                    self.waiting_for_confirm.send(Ok(())).unwrap();

                    let remote_confirm_code = self.confirm_code_channel.await?;

                    debug!("received confirm code from user: {remote_confirm_code}");

                    if confirm_code != remote_confirm_code {
                        return Err(anyhow!("Confirm Code did no match"));
                    }

                    debug!("Confirm Codes match!");

                    let raw_request: String = session_e2e.read_object().await?;
                    debug!("received request: {}", raw_request);
                    let request = CertificateRequest::from_string(raw_request)?;
                    let agent_cert: Certificate = self.credentials.approve_request(request)?;

                    session_e2e.write_object(&agent_cert).await?;
                    session_e2e.write_object(self.root).await?;
                    session_e2e.write_object(self.upstream).await?;

                    Ok(agent_cert)
                }
            }
        } else {
            Err(anyhow!("tried dispatching command with None"))
        }
    }
}

#[instrument(skip_all)]
async fn prepare_agent_enroll(
    mut session: Session,
    join_code: String,
    credentials: &PermCredentials,
) -> anyhow::Result<(String, Session)> {
    session.write_object(&join_code).await?;

    let found: std::result::Result<(), ()> = session.read_object().await?;

    if let Err(()) = found {
        return Err(anyhow!("Agent not found"));
    }

    debug!("connected to agent, sending join code for confirmation");

    // confirm join code with agent
    session.write_object(&join_code).await?;

    debug!("waiting for agent to confirm join code");

    let ready: std::result::Result<(), ()> = session.read_object().await?;

    if let Err(()) = ready {
        return Err(anyhow!("Agent did not aknowledge connection"));
    }

    debug!("agent confirmed join code");

    // establish tls session

    debug!("trying to establish tls connection");

    let (read, write, _) = session.destructure();

    let tls_transport = TlsTransport::client(
        CombinedTransport::new(read, write),
        SkipServerVerification::new(),
        credentials,
    )
    .await?;

    let mut key_material = [0u8; 32];
    tls_transport
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
