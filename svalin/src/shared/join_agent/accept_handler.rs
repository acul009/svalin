use anyhow::anyhow;
use async_trait::async_trait;
use svalin_macros::rpc_dispatch;
use svalin_pki::{ArgonParams, PermCredentials};
use svalin_rpc::{
    rpc::{
        command::CommandHandler,
        session::{Session, SessionOpen},
    },
    skip_verify::SkipServerVerification,
    transport::tls_transport::TlsTransport,
};

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

        let answer = match agent_session {
            Some(_) => Ok(()),
            None => Err(()),
        };

        session.write_object(&answer).await?;

        match agent_session {
            Some(mut agent_session) => {
                let answer: Result<(), ()> = Ok(());
                session.write_object(&answer).await?;

                session.forward(&mut agent_session).await?;

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
async fn accept_join(
    session: &mut Session<SessionOpen>,
    join_code: String,
    confirm_code_channel: tokio::sync::oneshot::Sender<String>,
    credentials: PermCredentials,
) -> anyhow::Result<()> {
    session.write_object(&join_code).await?;

    let found: std::result::Result<(), ()> = session.read_object().await?;

    if let Err(()) = found {
        return Err(anyhow!("Agent not found"));
    }

    // establish tls session

    let ready: std::result::Result<(), ()> = session.read_object().await?;

    if let Err(()) = ready {
        return Err(anyhow!("Agent did not aknowledge connection"));
    }

    let (key_material_send, key_material_recv) = tokio::sync::oneshot::channel::<[u8; 32]>();

    session
        .replace_transport(move |direct_transport| async move {
            let tls_transport =
                TlsTransport::client(direct_transport, SkipServerVerification::new(), credentials)
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

    let params = ArgonParams::basic();

    session.write_object(&params).await?;

    let confirm_code = super::derive_confirm_code(params, &key_material).await?;

    todo!()
}
