use std::ops::Deref;

use anyhow::anyhow;
use dispatcher::TcpForwardDispatcher;
use svalin_rpc::rpc::connection::Connection;
use thiserror::Error;
use tokio::{
    net::TcpListener,
    select,
    sync::{oneshot, watch},
};

use super::TunnelConfig;

pub mod dispatcher;
pub mod handler;

#[derive(Debug, Clone)]
pub struct TcpTunnelConfig {
    pub local_port: u16,
    pub remote_host: String,
}

impl From<TcpTunnelConfig> for TunnelConfig {
    fn from(config: TcpTunnelConfig) -> Self {
        Self::Tcp(config)
    }
}

#[derive(Debug, Error)]
pub enum TcpTunnelCreateError {
    #[error("Failed to bind to port {0} with error: {1}")]
    BindError(u16, #[source] std::io::Error),
}

#[derive(Debug, Error)]
pub enum TcpTunnelRunError {
    #[error("failed to accept connection: {0}")]
    AcceptConnectionError(#[source] std::io::Error),
}

impl TcpTunnelConfig {
    pub async fn run(
        &self,
        connection: impl Connection + 'static,
        mut active_recv: watch::Receiver<bool>,
    ) -> Result<oneshot::Receiver<TcpTunnelRunError>, TcpTunnelCreateError> {
        let config = self.clone();

        let listener = TcpListener::bind(format!("0.0.0.0:{}", config.local_port))
            .await
            .map_err(|err| TcpTunnelCreateError::BindError(config.local_port, err))?;

        let (error_send, error_recv) = oneshot::channel();

        tokio::spawn(async move {
            loop {
                select! {
                    stream = listener.accept() => {
                        if !*active_recv.borrow().deref() {
                            return;
                        }
                        match stream {
                            Err(err) => {
                                let _ = error_send.send(TcpTunnelRunError::AcceptConnectionError(err));
                                return;
                            }
                            Ok((stream, _)) => {

                                let connection = connection.clone();
                                let dispatcher = TcpForwardDispatcher {
                                    active: active_recv.clone(),
                                    target: config.remote_host.clone(),
                                    stream,
                                };

                                tokio::spawn(async move {
                                    if let Err(err) = connection.dispatch(dispatcher).await {
                                        let err = anyhow!(err).context("error running tcp tunnel");
                                        tracing::error!("{:#}", err);
                                    }
                                });
                            }
                        }
                    }
                    _ = active_recv.changed() => {
                        if !*active_recv.borrow().deref() {
                            return;
                        }
                    }
                }
            }
        });

        Ok(error_recv)
    }
}
