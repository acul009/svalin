use std::{mem, time::Duration};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use rand::Rng;
use svalin_macros::rpc_dispatch;
use svalin_pki::{Certificate, Keypair};
use svalin_rpc::{
    rpc::{
        command::CommandHandler,
        session::{Session, SessionOpen},
    },
    skip_verify::SkipClientVerification,
    transport::tls_transport::TlsTransport,
};
use tokio::{io::AsyncWriteExt, sync::oneshot};
use tracing::{debug, error};

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
    rand::thread_rng().gen_range(0..999999).to_string()
}

fn join_request_key() -> String {
    "join_request".to_string()
}

#[async_trait]
impl CommandHandler for JoinRequestHandler {
    fn key(&self) -> String {
        join_request_key()
    }

    async fn handle(&self, session: &mut Session<SessionOpen>) -> Result<()> {
        let mut add_session = mem::replace(session, Session::dangerous_create_dummy_session());

        let mut join_code = create_join_code();
        while let Err(sess) = self.manager.add_session(join_code, add_session).await {
            add_session = sess;
            tokio::time::sleep(Duration::from_secs(5)).await;

            join_code = create_join_code();

            // todo: dont loop forever
        }

        Ok(())
    }
}

#[rpc_dispatch(join_request_key())]
#[instrument(skip_all)]
pub async fn request_join(
    session: &mut Session<SessionOpen>,
    address: String,
    join_code_channel: oneshot::Sender<String>,
    confirm_code_channel: oneshot::Sender<String>,
) -> Result<AgentInitPayload> {
    let join_code: String = session.read_object().await?;

    debug!("received join code from server: {join_code}");

    join_code_channel
        .send(join_code.clone())
        .map_err(|err| anyhow!(err))?;

    debug!("waiting for client to confirm join code");

    let join_code_confirm: String = session.read_object().await?;

    debug!("received join code from client: {join_code_confirm}");

    if join_code != join_code_confirm {
        debug!("join codes do not match!");
        let answer: Result<(), ()> = Err(());
        session.write_object(&answer).await?;
        return Err(anyhow!("Invalid join code"));
    } else {
        debug!("join codes match!");
        let answer: Result<(), ()> = Ok(());
        session.write_object(&answer).await?;
    }

    debug!("trying to establish tls connection");

    let mut key_material_result: Result<[u8; 32]> = Err(anyhow!("unknown tls error"));
    let key_material_result_borrow = &mut key_material_result;

    session
        .replace_transport(move |mut direct_transport| async move {
            if let Err(err) = direct_transport.flush().await {
                error!("error while replacing transport: {}", err);
            }
            let temp_credentials = Keypair::generate().unwrap().to_self_signed_cert().unwrap();

            let tls_transport = TlsTransport::server(
                direct_transport,
                SkipClientVerification::new(),
                &temp_credentials,
            )
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

    let key_material = key_material_result.context("error during tls handshake on agent")?;

    debug!("server tls connection established");

    let params = session.read_object().await?;

    let confirm_code = super::derive_confirm_code(params, &key_material).await?;

    debug!("generated confirm code: {confirm_code}");

    confirm_code_channel.send(confirm_code).unwrap();

    let keypair = Keypair::generate()?;
    let request = keypair.generate_request()?;
    debug!("sending request: {}", request);
    session.write_object(&request).await?;

    let my_cert: Certificate = session.read_object().await?;
    let my_credentials = keypair.upgrade(my_cert)?;

    let root: Certificate = session.read_object().await?;
    let upstream: Certificate = session.read_object().await?;

    Ok(AgentInitPayload {
        credentials: my_credentials,
        address,
        root,
        upstream,
    })
}
