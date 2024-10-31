use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rand::Rng;
use svalin_pki::{Certificate, Keypair};
use svalin_rpc::{
    rpc::{
        command::{dispatcher::TakeableCommandDispatcher, handler::TakeableCommandHandler},
        peer::Peer,
        session::Session,
    },
    transport::{combined_transport::CombinedTransport, tls_transport::TlsTransport},
    verifiers::skip_verify::SkipClientVerification,
};
use tokio::sync::oneshot;
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
    rand::thread_rng().gen_range(0..999999).to_string()
}

#[async_trait]
impl TakeableCommandHandler for JoinRequestHandler {
    type Request = ();

    fn key() -> String {
        "join_request".to_string()
    }

    async fn handle(&self, session: &mut Option<Session>, _: Self::Request) -> Result<()> {
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

pub struct RequestJoin {
    pub address: String,
    pub join_code_channel: oneshot::Sender<String>,
    pub confirm_code_channel: oneshot::Sender<String>,
}

#[async_trait]
impl TakeableCommandDispatcher for RequestJoin {
    type Output = AgentInitPayload;

    type Request = ();

    fn key() -> String {
        JoinRequestHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
        _: Self::Request,
    ) -> Result<Self::Output> {
        if let Some(mut session) = session.take() {
            let join_code: String = session.read_object().await?;

            debug!("received join code from server: {join_code}");

            self.join_code_channel
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

            let (read, write, _) = session.destructure_transport();

            let temp_credentials = Keypair::generate().unwrap().to_self_signed_cert()?;

            let tls_transport = TlsTransport::server(
                CombinedTransport::new(read, write),
                SkipClientVerification::new(),
                &temp_credentials,
            )
            .await?;

            let mut key_material = [0u8; 32];
            tls_transport
                .derive_key(&mut key_material, b"join_confirm_key", join_code.as_bytes())
                .unwrap();

            let (read, write) = tokio::io::split(tls_transport);

            let mut session = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

            debug!("server tls connection established");

            let params = session.read_object().await?;

            let confirm_code = super::derive_confirm_code(params, &key_material).await?;

            debug!("generated confirm code: {confirm_code}");

            self.confirm_code_channel.send(confirm_code).unwrap();

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
                address: self.address,
                root,
                upstream,
            })
        } else {
            Err(anyhow!("tried dispatching command with None"))
        }
    }
}
