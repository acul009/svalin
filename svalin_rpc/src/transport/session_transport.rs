use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait]
pub trait SessionTransport: AsyncRead + AsyncWrite + Send + Unpin + Send + Sync {
    async fn stopped(&mut self);
}
