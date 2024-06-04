use std::ops::DerefMut;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait]
pub trait SessionTransport: AsyncRead + AsyncWrite + Send + Unpin + Send + Sync {}

#[async_trait]
impl SessionTransport for Box<dyn SessionTransport> {}
