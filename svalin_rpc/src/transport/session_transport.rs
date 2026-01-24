use std::any::Any;

use tokio::io::{AsyncRead, AsyncWrite};

pub trait SessionTransport: SessionTransportReader + SessionTransportWriter {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}
impl<T> SessionTransport for T
where
    T: SessionTransportReader + SessionTransportWriter + 'static,
{
    fn into_any(self: Box<Self>) -> Box<dyn Any + 'static> {
        self
    }
}

pub trait SessionTransportReader: AsyncRead + Send + Unpin + Send + Sync {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}
impl<T> SessionTransportReader for T
where
    T: AsyncRead + Send + Unpin + Send + Sync + 'static,
{
    fn into_any(self: Box<Self>) -> Box<dyn Any + 'static> {
        self
    }
}

pub trait SessionTransportWriter: AsyncWrite + Send + Unpin + Send + Sync {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}
impl<T> SessionTransportWriter for T
where
    T: AsyncWrite + Send + Unpin + Send + Sync + 'static,
{
    fn into_any(self: Box<Self>) -> Box<dyn Any + 'static> {
        self
    }
}

// #[async_trait]
// impl SessionTransport for Box<dyn SessionTransport> {}
