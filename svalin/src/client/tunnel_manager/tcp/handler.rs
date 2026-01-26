use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::{
    command::handler::{CommandHandler, PermissionPrecursor},
    session::Session,
};
use thiserror::Error;
use tokio::{
    io::{AsyncWriteExt, copy_bidirectional},
    net::TcpStream,
    select,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use crate::permissions::Permission;

#[derive(Default)]
pub struct TcpForwardHandler;

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum TcpForwardError {
    #[error("Failed to connect to connect to requested target")]
    Generic,
}

impl From<&PermissionPrecursor<TcpForwardHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<TcpForwardHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

#[async_trait]
impl CommandHandler for TcpForwardHandler {
    type Request = String;

    fn key() -> String {
        "tcp_forward".to_string()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> Result<()> {
        debug!("incoming tcp_forward request: {request}");
        let stream = TcpStream::connect(request).await;

        match stream {
            Err(err) => {
                debug!("failed to connect: {err}");
                session
                    .write_object::<Result<(), TcpForwardError>>(&Err(TcpForwardError::Generic))
                    .await?;

                return Err(err.into());
            }
            Ok(mut stream) => {
                debug!("connected!");
                session
                    .write_object::<Result<(), TcpForwardError>>(&Ok(()))
                    .await?;

                let mut transport = session.borrow_transport();

                debug!("copying!");

                select! {
                    _ = cancel.cancelled() => {

                    }
                    result = copy_bidirectional(&mut stream, &mut transport) => {
                        result?;
                    }
                }

                if let Err(err) = stream.shutdown().await {
                    error!("error shutting down stream: {err}");
                }
                if let Err(err) = transport.shutdown().await {
                    error!("error shutting down transport: {err}");
                }

                Ok(())
            }
        }
    }
}
