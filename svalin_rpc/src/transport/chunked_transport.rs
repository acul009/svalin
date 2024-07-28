use std::mem;

use anyhow::{anyhow, Ok, Result};
use futures::Future;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{dummy_transport::DummyTransport, session_transport::SessionTransport};

pub(super) struct ChunkedTransport {
    transport: Box<dyn SessionTransport>,
}

impl ChunkedTransport {
    pub(super) fn new(transport: Box<dyn SessionTransport>) -> Self {
        Self { transport }
    }

    pub async fn write_chunk(&mut self, chunk: &[u8]) -> Result<()> {
        let len = chunk.len();
        if len > 1 << 31 {
            // error
            return Err(anyhow!("The given data chunk is to big"));
        }
        // println!("original chunk: {:?}", chunk);
        let len: u32 = len.try_into().unwrap();
        if len < 0b1000_0000 {
            let lenbytes = len.to_be_bytes();
            // println!("using short len: {}", lenbytes[3]);
            self.transport.write_u8(lenbytes[3]).await?;
        } else {
            let lenbytes = (len | (1 << 31)).to_be_bytes();
            self.transport.write_all(&lenbytes).await?;
        }

        self.transport.write_all(chunk).await?;
        Ok(())
    }

    pub async fn read_chunk(&mut self) -> Result<Vec<u8>> {
        let short_len = self.transport.read_u8().await?;

        let len: usize;

        // println!("read short len: {}", short_len);

        if short_len < 0b1000_0000 {
            len = short_len.into()
        } else {
            // length is 4 bytes
            // println!("use long len");
            let mut size_be = [short_len & 0b0111_1111, 0, 0, 0];
            self.transport.read_exact(&mut size_be[1..]).await?;
            len = u32::from_be_bytes(size_be) as usize;
        }

        let mut chunk = vec![0; len];

        self.transport.read_exact(&mut chunk).await?;

        // println!("read chunk: {:?}", &chunk);

        Ok(chunk)
    }

    pub async fn replace_transport<R, Fut>(&mut self, replacer: R)
    where
        R: FnOnce(Box<dyn SessionTransport>) -> Fut,
        Fut: Future<Output = Box<dyn SessionTransport>>,
    {
        let transport = mem::replace(&mut self.transport, Box::new(DummyTransport::new()));

        let new_transport = replacer(transport).await;

        let _ = mem::replace(&mut self.transport, new_transport);
    }

    pub async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        self.transport.shutdown().await
    }

    pub fn borrow_transport(&mut self) -> &mut Box<dyn SessionTransport> {
        &mut self.transport
    }

    pub fn extract_transport(self) -> Box<dyn SessionTransport> {
        self.transport
    }
}
