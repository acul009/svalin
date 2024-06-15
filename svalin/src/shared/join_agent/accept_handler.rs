use std::mem;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use svalin_macros::rpc_dispatch;
use svalin_pki::{ArgonParams, Certificate, CertificateRequest, PermCredentials};
use svalin_rpc::{
    rpc::{
        command::CommandHandler,
        session::{Session, SessionOpen},
    },
    skip_verify::SkipServerVerification,
    transport::tls_transport::TlsTransport,
};
use tokio::io::AsyncWriteExt;
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

    async fn handle(&self, session: &mut Session<SessionOpen>) -> anyhow::Result<()> {
        let join_code: String = session.read_object().await?;

        let agent_session = self.manager.get_session(&join_code).await;

        match agent_session {
            Some(mut agent_session) => {
                let answer: Result<(), ()> = Ok(());
                session.write_object(&answer).await?;

                debug!("forwarding session to agent");
                session.forward(&mut agent_session).await?;

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

#[rpc_dispatch(accept_join_code())]
pub async fn accept_join(
    session: &mut Session<SessionOpen>,
    join_code: String,
    waiting_for_confirm: tokio::sync::oneshot::Sender<Result<()>>,
    confirm_code_channel: tokio::sync::oneshot::Receiver<String>,
    credentials: &PermCredentials,
    root: &Certificate,
    upstream: &Certificate,
) -> anyhow::Result<Certificate> {
    let confirm_code_result = prepare_agent_enroll(session, join_code, credentials).await;

    match confirm_code_result {
        Err(err) => {
            let err_copy = anyhow!("{}", err);
            let err = err.context("error during enroll");
            waiting_for_confirm.send(Err(err)).unwrap();

            Err(err_copy)
        }
        Ok(confirm_code) => {
            waiting_for_confirm.send(Ok(())).unwrap();

            let remote_confirm_code = confirm_code_channel.await?;

            debug!("received confirm code from user: {remote_confirm_code}");

            if confirm_code != remote_confirm_code {
                return Err(anyhow!("Confirm Code did no match"));
            }

            debug!("Confirm Codes match!");

            let raw_request: String = session.read_object().await?;
            debug!("received request: {}", raw_request);
            let request = CertificateRequest::from_string(raw_request)?;
            let agent_cert: Certificate = credentials.approve_request(request)?;

            session.write_object(&agent_cert).await?;
            session.write_object(root).await?;
            session.write_object(upstream).await?;

            Ok(agent_cert)
        }
    }
}

#[instrument(skip_all)]
async fn prepare_agent_enroll(
    session: &mut Session<SessionOpen>,
    join_code: String,
    credentials: &PermCredentials,
) -> anyhow::Result<String> {
    session.write_object(&join_code).await?;

    let found: std::result::Result<(), ()> = session.read_object().await?;

    if let Err(()) = found {
        return Err(anyhow!("Agent not found"));
    }

    debug!("connected to agent");

    // confirm join code with agent
    session.write_object(&join_code).await?;

    let ready: std::result::Result<(), ()> = session.read_object().await?;

    if let Err(()) = ready {
        return Err(anyhow!("Agent did not aknowledge connection"));
    }

    debug!("agent confirmed join code");

    // establish tls session

    debug!("trying to establish tls connection");

    let mut key_material_result: Result<[u8; 32]> = Err(anyhow!("unknown tls error"));
    let key_material_result_borrow = &mut key_material_result;

    session
        .replace_transport(move |mut direct_transport| async move {
            direct_transport.flush().await;
            let tls_transport =
                TlsTransport::client(direct_transport, SkipServerVerification::new(), credentials)
                    .await;

            match tls_transport {
                Ok(tls_transport) => {
                    let mut key_material = [0u8; 32];
                    tls_transport
                        .derive_key(&mut key_material, b"join_confirm_key", join_code.as_bytes())
                        .unwrap();

                    let _ = mem::replace(key_material_result_borrow, Ok(key_material));
                    Box::new(tls_transport)
                }
                Err(err) => {
                    let _ = mem::replace(key_material_result_borrow, Err(err.0));
                    err.1
                }
            }
        })
        .await;

    let key_material = key_material_result.context("error during tls handshake on client")?;

    debug!("client tls connection established");

    let params = ArgonParams::basic();

    session.write_object(&params).await?;

    let confirm_code = super::derive_confirm_code(params, &key_material).await?;

    debug!("client side confirm code: {confirm_code}");

    Ok(confirm_code)
}
