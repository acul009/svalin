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
        aucpace_transport::{AucPaceClientError, AucPaceTransport},
        combined_transport::CombinedTransport,
    },
};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::rpc::session::Session;

pub struct AucPaceTestCommandHandler {}

impl AucPaceTestCommandHandler {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

const TEST_PASSWORD: &[u8] = b"Affenbrotbaum!";

#[async_trait]
impl TakeableCommandHandler for AucPaceTestCommandHandler {
    type Request = ();

    fn key() -> String {
        "auc_pace_test".to_string()
    }

    async fn handle(
        &self,
        session: &mut Option<Session>,
        _: Self::Request,
        _: CancellationToken,
    ) -> anyhow::Result<()> {
        if let Some(session_ready) = session.take() {
            let (read, write, _) = session_ready.destructure_transport();

            let transport = AucPaceTransport::server(
                CombinedTransport::new(read, write),
                TEST_PASSWORD.to_vec(),
            )
            .await?;

            let (read, write) = tokio::io::split(transport);

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
pub enum AucPaceTestClientError {
    #[error("error during TLS client test: {0}")]
    AucPaceClientError(#[from] AucPaceClientError),
    #[error("error writing ping: {0}")]
    WritePingError(#[from] SessionWriteError),
    #[error("error reading pong: {0}")]
    ReadPongError(#[from] SessionReadError),
    #[error("error converting timestamp: {0}")]
    ParseTimestampError(#[from] std::num::TryFromIntError),
}

pub struct AucPaceTest;

impl TakeableCommandDispatcher for AucPaceTest {
    type Output = ();

    type InnerError = AucPaceTestClientError;

    type Request = ();

    fn key() -> String {
        AucPaceTestCommandHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
    ) -> Result<Self::Output, DispatcherError<Self::InnerError>> {
        if let Some(session_ready) = session.take() {
            let (read, write, _) = session_ready.destructure_transport();

            let transport = AucPaceTransport::client(
                CombinedTransport::new(read, write),
                TEST_PASSWORD.to_vec(),
            )
            .await
            .map_err(AucPaceTestClientError::AucPaceClientError)?;

            let (read, write) = tokio::io::split(transport);

            let mut session_ready = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

            let ping = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_nanos();

            session_ready
                .write_object(&ping)
                .await
                .map_err(AucPaceTestClientError::WritePingError)?;

            let pong: u128 = session_ready
                .read_object()
                .await
                .map_err(AucPaceTestClientError::ReadPongError)?;

            let now: u128 = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_nanos();

            let diff = Duration::from_nanos(
                (now - pong)
                    .try_into()
                    .map_err(AucPaceTestClientError::ParseTimestampError)?,
            );

            println!("TLS-Ping: {:?}", diff);

            *session = Some(session_ready);
            Ok(())
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}
