use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    rpc::{
        command::{dispatcher::TakeableCommandDispatcher, handler::TakeableCommandHandler},
        peer::Peer,
    },
    transport::{combined_transport::CombinedTransport, tls_transport::TlsTransport},
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use svalin_pki::Keypair;
use tokio_util::sync::CancellationToken;

use crate::rpc::session::Session;

pub struct TlsTestCommandHandler {}

impl TlsTestCommandHandler {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

fn tls_test_key() -> String {
    "tls_test".into()
}

#[async_trait]
impl TakeableCommandHandler for TlsTestCommandHandler {
    type Request = ();

    fn key() -> String {
        tls_test_key()
    }

    async fn handle(
        &self,
        session: &mut Option<Session>,
        _: Self::Request,
        _: CancellationToken,
    ) -> anyhow::Result<()> {
        if let Some(session_ready) = session.take() {
            let (read, write, _) = session_ready.destructure_transport();

            let credentials = Keypair::generate().unwrap().to_self_signed_cert().unwrap();

            let tls_transport = TlsTransport::server(
                CombinedTransport::new(read, write),
                crate::verifiers::skip_verify::SkipClientVerification::new(),
                &credentials,
            )
            .await?;

            let (read, write) = tokio::io::split(tls_transport);

            let mut session_ready = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

            let ping: u64 = session_ready.read_object().await?;
            session_ready.write_object(&ping).await?;

            *session = Some(session_ready);

            Ok(())
        } else {
            Err(anyhow!("no session given"))
        }
    }
}

pub struct TlsTest;

#[async_trait]
impl TakeableCommandDispatcher for TlsTest {
    type Output = ();

    type Request = ();

    fn key() -> String {
        tls_test_key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(self, session: &mut Option<Session>, _: Self::Request) -> Result<()> {
        if let Some(session_ready) = session.take() {
            let (read, write, _) = session_ready.destructure_transport();

            let credentials = Keypair::generate().unwrap().to_self_signed_cert().unwrap();

            let tls_transport = TlsTransport::client(
                CombinedTransport::new(read, write),
                crate::verifiers::skip_verify::SkipServerVerification::new(),
                &credentials,
            )
            .await?;

            let (read, write) = tokio::io::split(tls_transport);

            let mut session_ready = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

            let ping = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_nanos();

            session_ready.write_object(&ping).await?;

            let pong: u128 = session_ready.read_object().await?;

            let now: u128 = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_nanos();

            let diff = Duration::from_nanos((now - pong).try_into()?);

            println!("TLS-Ping: {:?}", diff);

            *session = Some(session_ready);
            Ok(())
        } else {
            Err(anyhow!("no session given"))
        }
    }
}
