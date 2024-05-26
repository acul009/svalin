use std::pin::Pin;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use super::session_transport::SessionTransport;

pub struct CombinedTransport<T, U> {
    send: T,
    recv: U,
}

#[async_trait]
impl<T, U> SessionTransport for CombinedTransport<T, U>
where
    T: Send + AsyncWrite + Unpin,
    U: Send + AsyncRead + Unpin,
{
    async fn stopped(&mut self) {
        todo!()
    }
}

impl<T, U> AsyncWrite for CombinedTransport<T, U>
where
    T: AsyncWrite + Unpin,
    U: Unpin,
{
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.send).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.send).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.send).poll_shutdown(cx)
    }
}

impl<T, U> AsyncRead for CombinedTransport<T, U>
where
    T: Unpin,
    U: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl<T, U> From<(T, U)> for CombinedTransport<T, U> {
    fn from(value: (T, U)) -> Self {
        CombinedTransport {
            send: value.0,
            recv: value.1,
        }
    }
}
