use std::pin::Pin;

use async_trait::async_trait;
use quinn::{RecvStream, SendStream};
use tokio::io::{AsyncRead, AsyncWrite};

use super::session_transport::SessionTransport;

pub struct CombinedTransport<S, R> {
    send: S,
    recv: R,
}

// #[async_trait]
// impl<T, U> SessionTransport for CombinedTransport<T, U>
// where
//     T: Send + AsyncWrite + Unpin,
//     U: Send + AsyncRead + Unpin,
// {
//     async fn stopped(&mut self) {
//         todo!()
//     }
// }

#[async_trait]
impl SessionTransport for CombinedTransport<SendStream, RecvStream> {}

impl<S, R> AsyncWrite for CombinedTransport<S, R>
where
    S: AsyncWrite + Unpin,
    R: Unpin,
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

impl<S, R> AsyncRead for CombinedTransport<S, R>
where
    S: Unpin,
    R: AsyncRead + Unpin,
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
