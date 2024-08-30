use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    self as svalin_rpc,
    rpc::{
        command::{
            dispatcher::{CommandDispatcher, TakeableCommandDispatcher},
            handler::TakeableCommandHandler,
        },
        peer::Peer,
    },
    transport::{
        combined_transport::CombinedTransport,
        tls_transport::{self, TlsTransport},
    },
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::future::ok;
use svalin_macros::rpc_dispatch;
use svalin_pki::{Keypair, PermCredentials};
use tracing::error;

use crate::rpc::{command::handler::CommandHandler, session::Session};

pub struct TlsTestCommandHandler {
    credentials: PermCredentials,
}

impl TlsTestCommandHandler {
    pub fn new() -> Result<Self> {
        let credentials = Keypair::generate()?.to_self_signed_cert()?;

        Ok(Self { credentials })
    }
}

fn tls_test_key() -> String {
    "tls_test".into()
}

#[async_trait]
impl TakeableCommandHandler for TlsTestCommandHandler {
    fn key(&self) -> String {
        tls_test_key()
    }

    async fn handle(&self, session: &mut Option<Session>) -> anyhow::Result<()> {
        if let Some(session_ready) = session.take() {
            let (read, write, _) = session_ready.destructure();

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

pub struct TlsTestDispatcher {}

impl TakeableCommandDispatcher<()> for TlsTestDispatcher {
    fn key(&self) -> String {
        tls_test_key()
    }
    async fn dispatch(&self, session: &mut Option<Session>) -> Result<()> {
        if let Some(session_ready) = session.take() {
            let (read, write, _) = session_ready.destructure();

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
