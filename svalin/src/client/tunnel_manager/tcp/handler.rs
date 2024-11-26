use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_rpc::{
    rpc::{command::handler::CommandHandler, session::Session},
    transport::combined_transport::CombinedTransport,
};
use thiserror::Error;
use tokio::{io::copy_bidirectional, net::TcpStream};

pub struct TcpForwardHandler {}

impl TcpForwardHandler {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum TcpForwardError {
    #[error("Failed to connect to connect to requested target")]
    Generic,
}

#[async_trait]
impl CommandHandler for TcpForwardHandler {
    type Request = String;

    fn key() -> String {
        "tcp_forward".to_string()
    }

    async fn handle(&self, session: &mut Session, request: Self::Request) -> Result<()> {
        let stream = TcpStream::connect(request).await;

        match stream {
            Err(err) => {
                session
                    .write_object::<Result<(), TcpForwardError>>(&Err(TcpForwardError::Generic))
                    .await?;

                return Err(err.into());
            }
            Ok(mut stream) => {
                let (transport_read, transport_write) = session.borrow_transport();
                let mut transport = CombinedTransport::new(transport_read, transport_write);

                copy_bidirectional(&mut transport, &mut stream).await?;

                Ok(())
            }
        }
    }
}
