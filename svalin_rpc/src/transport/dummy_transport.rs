use tokio::io::{AsyncRead, AsyncWrite};

use super::session_transport::SessionTransport;

pub(crate) struct DummyTransport {}

impl Default for DummyTransport {
    fn default() -> Self {
        Self {}
    }
}

impl SessionTransport for DummyTransport {}

impl AsyncWrite for DummyTransport {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        unreachable!()
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        unreachable!()
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        unreachable!()
    }
}

impl AsyncRead for DummyTransport {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        unreachable!()
    }
}
