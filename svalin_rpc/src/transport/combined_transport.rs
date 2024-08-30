use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};

/// Helper transport which can be used for TLS wrapping
pub struct CombinedTransport<R, W> {
    read: R,
    write: W,
}

impl<R, W> CombinedTransport<R, W> {
    pub fn new(read: R, write: W) -> Self {
        Self { read, write }
    }

    pub fn split(self) -> (R, W) {
        (self.read, self.write)
    }
}

impl<R, W> AsyncWrite for CombinedTransport<R, W>
where
    W: AsyncWrite + Unpin,
    R: Unpin,
{
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.write).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.write).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.write).poll_shutdown(cx)
    }
}

impl<R, W> AsyncRead for CombinedTransport<R, W>
where
    W: Unpin,
    R: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.read).poll_read(cx, buf)
    }
}

impl<T, U> From<(T, U)> for CombinedTransport<T, U> {
    fn from(value: (T, U)) -> Self {
        CombinedTransport {
            read: value.0,
            write: value.1,
        }
    }
}
