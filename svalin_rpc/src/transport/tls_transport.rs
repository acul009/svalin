use std::sync::Arc;

use anyhow::Result;
use futures::AsyncRead;
use quinn::rustls;
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    stream,
};
use tokio_rustls::{TlsAcceptor, TlsStream};

use super::session_transport::SessionTransport;

pub struct TlsTransport<T>
where
    T: SessionTransport,
{
    tls_stream: TlsStream<T>,
}

impl<T> TlsTransport<T>
where
    T: SessionTransport,
{
    pub async fn client(base_transport: T) -> Result<Self> {
        let config: Arc<quinn::rustls::ClientConfig> = todo!();
        let name = todo!();

        let mut connector = tokio_rustls::TlsConnector::from(config);

        let hostname = tokio_rustls::rustls::pki_types::ServerName::try_from("todo")?;
        let client = connector.connect(hostname, base_transport).await?;

        let tls_stream = TlsStream::Client(client);

        Ok(Self { tls_stream })
    }

    pub async fn server(base_transport: T) -> Result<Self> {
        let config: Arc<quinn::rustls::ServerConfig> = todo!();

        let acceptor = TlsAcceptor::from(config);

        let mut server = acceptor.accept(base_transport).await?;

        let tls_stream = TlsStream::Server(server);

        Ok(Self { tls_stream })
    }
}
