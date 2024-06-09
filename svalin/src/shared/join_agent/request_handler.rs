use std::time::Duration;

use anyhow::{anyhow, Result};
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
use tokio::sync::oneshot;

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
        let add_session: Session<SessionOpen> = todo!();
        let mut join_code = create_join_code();
        while let Err(sess) = self.manager.add_session(join_code, add_session).await {
            add_session = sess;
            tokio::time::sleep(Duration::from_secs(5)).await;

            join_code = create_join_code();

            //todo: dont loop forever
        }

        Ok(())
    }
}

#[rpc_dispatch(join_request_key())]
pub async fn request_join(
    session: &mut Session<SessionOpen>,
    join_code_channel: oneshot::Sender<String>,
    confirm_code_channel: oneshot::Sender<String>,
) -> Result<AgentInitPayload> {
    let join_code: String = session.read_object().await?;
    join_code_channel
        .send(join_code.clone())
        .map_err(|err| anyhow!(err))?;

    let join_code_confirm: String = session.read_object().await?;

    if join_code != join_code_confirm {
        let answer: Result<(), ()> = Err(());
        session.write_object(&answer).await?;
        return Err(anyhow!("Invalid join code"));
    } else {
        let answer: Result<(), ()> = Ok(());
        session.write_object(&answer).await?;
    }

    let (key_material_send, key_material_recv) = tokio::sync::oneshot::channel::<[u8; 32]>();

    session
        .replace_transport(move |direct_transport| async move {
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
                    key_material_send.send(key_material).unwrap();
                    Box::new(tls_transport)
                }
                Err(err) => err.1,
            }
        })
        .await;

    let key_material = key_material_recv.await?;

    let params = session.read_object().await?;

    let confirm_code = super::derive_confirm_code(params, &key_material).await?;

    confirm_code_channel.send(confirm_code).unwrap();

    let keypair = Keypair::generate()?;
    let request = keypair.generate_request()?;
    session.write_object(&request).await?;

    let my_cert: Certificate = session.read_object().await?;
    let my_credentials = keypair.upgrade(my_cert)?;

    let root: Certificate = session.read_object().await?;
    let upstream: Certificate = session.read_object().await?;

    Ok(AgentInitPayload {
        credentials: my_credentials,
        root,
        upstream,
    })
}
