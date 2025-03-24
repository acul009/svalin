use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    rpc::{
        command::{
            dispatcher::{DispatcherError, TakeableCommandDispatcher},
            handler::TakeableCommandHandler,
        },
        peer::Peer,
        session::{SessionReadError, SessionWriteError},
    },
    transport::{
        combined_transport::CombinedTransport,
        tls_transport::{TlsClientError, TlsTransport},
    },
};
use anyhow::{Result, anyhow};
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

            let credentials = Keypair::generate().to_self_signed_cert().unwrap();

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

#[derive(Debug, thiserror::Error)]
pub enum TlsTestClientError {
    #[error("error during TLS client test: {0}")]
    TlsClientError(#[from] TlsClientError),
    #[error("error writing ping: {0}")]
    WritePingError(#[from] SessionWriteError),
    #[error("error reading pong: {0}")]
    ReadPongError(#[from] SessionReadError),
    #[error("error converting timestamp: {0}")]
    ParseTimestampError(#[from] std::num::TryFromIntError),
}

pub struct TlsTest;

#[async_trait]
impl TakeableCommandDispatcher for TlsTest {
    type Output = ();

    type InnerError = TlsTestClientError;

    type Request = ();

    fn key() -> String {
        tls_test_key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
        _: Self::Request,
    ) -> Result<Self::Output, DispatcherError<Self::InnerError>> {
        if let Some(session_ready) = session.take() {
            let (read, write, _) = session_ready.destructure_transport();

            let credentials = Keypair::generate().to_self_signed_cert().unwrap();

            let tls_transport = TlsTransport::client(
                CombinedTransport::new(read, write),
                crate::verifiers::skip_verify::SkipServerVerification::new(),
                &credentials,
            )
            .await
            .map_err(TlsTestClientError::TlsClientError)?;

            let (read, write) = tokio::io::split(tls_transport);

            let mut session_ready = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

            let ping = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_nanos();

            session_ready
                .write_object(&ping)
                .await
                .map_err(TlsTestClientError::WritePingError)?;

            let pong: u128 = session_ready
                .read_object()
                .await
                .map_err(TlsTestClientError::ReadPongError)?;

            let now: u128 = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_nanos();

            let diff = Duration::from_nanos(
                (now - pong)
                    .try_into()
                    .map_err(TlsTestClientError::ParseTimestampError)?,
            );

            println!("TLS-Ping: {:?}", diff);

            *session = Some(session_ready);
            Ok(())
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}
