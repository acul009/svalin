use std::{pin::Pin, sync::Arc};

use crate::rustls;
use anyhow::Result;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
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

        let hostname = rustls::pki_types::ServerName::try_from("todo")?;
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

#[async_trait]
impl<T: SessionTransport> SessionTransport for TlsTransport<T> {
    async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        self.tls_stream.shutdown().await
    }
}

impl<T: SessionTransport> AsyncRead for TlsTransport<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.tls_stream).poll_read(cx, buf)
    }
}

impl<T: SessionTransport> AsyncWrite for TlsTransport<T> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.tls_stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.tls_stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.tls_stream).poll_shutdown(cx)
    }
}
