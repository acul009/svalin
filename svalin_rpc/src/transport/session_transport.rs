use tokio::io::{AsyncRead, AsyncWrite};

pub trait SessionTransport: SessionTransportReader + SessionTransportWriter {}
impl<T> SessionTransport for T where T: SessionTransportReader + SessionTransportWriter {}

pub trait SessionTransportReader: AsyncRead + Send + Unpin + Send + Sync {}
impl<T> SessionTransportReader for T where T: AsyncRead + Send + Unpin + Send + Sync {}

pub trait SessionTransportWriter: AsyncWrite + Send + Unpin + Send + Sync {}
impl<T> SessionTransportWriter for T where T: AsyncWrite + Send + Unpin + Send + Sync {}

// #[async_trait]
// impl SessionTransport for Box<dyn SessionTransport> {}
