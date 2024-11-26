use std::{ops::Deref, sync::Arc};

use dispatcher::TcpForwardDispatcher;
use svalin_rpc::rpc::connection::Connection;
use thiserror::Error;
use tokio::{
    net::TcpListener,
    select,
    sync::{broadcast, oneshot, watch},
};

use super::{TunnelCreateError, TunnelRunError};

pub mod dispatcher;
pub mod handler;

#[derive(Clone)]
pub struct TcpTunnelConfig {
    local_port: u16,
    remote_host: String,
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
    pub fn run(
        &self,
        connection: impl Connection + 'static,
        mut active_recv: watch::Receiver<bool>,
    ) -> (
        oneshot::Receiver<Result<(), TcpTunnelCreateError>>,
        oneshot::Receiver<TcpTunnelRunError>,
    ) {
        let (running, running_recv) = oneshot::channel();
        let (error_send, error_recv) = oneshot::channel();
        let config = self.clone();

        tokio::spawn(async move {
            let listener = TcpListener::bind(format!("0.0.0.0:{}", config.local_port)).await;

            match listener {
                Err(err) => {
                    let _ =
                        running.send(Err(TcpTunnelCreateError::BindError(config.local_port, err)));
                }
                Ok(listener) => {
                    let _ = running.send(Ok(()));

                    select! {
                        stream = listener.accept() => {
                            if *active_recv.borrow().deref() {
                                return;
                            }
                            match stream {
                                Err(err) => {
                                    let _ = error_send.send(TcpTunnelRunError::AcceptConnectionError(err));
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
                                            tracing::error!("{err}");
                                        }
                                    });
                                }
                            }
                        }
                        _ = active_recv.changed() => {
                            if *active_recv.borrow().deref() {
                                return;
                            }
                        }
                    }
                }
            }
        });

        (running_recv, error_recv)
    }
}
