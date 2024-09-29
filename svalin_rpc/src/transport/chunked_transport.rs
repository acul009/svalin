use anyhow::{anyhow, Ok, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::session_transport::{SessionTransportReader, SessionTransportWriter};

pub(crate) struct ChunkReader {
    read: Box<dyn SessionTransportReader>,
}

pub(crate) struct ChunkWriter {
    write: Box<dyn SessionTransportWriter>,
}

impl ChunkReader {
    pub(crate) fn new(read: Box<dyn SessionTransportReader>) -> Self {
        Self { read }
    }

    pub async fn read_chunk(&mut self) -> Result<Vec<u8>> {
        let short_len = self.read.read_u8().await?;

        let len: usize;

        // println!("read short len: {}", short_len);

        if short_len < 0b1000_0000 {
            len = short_len.into()
        } else {
            // length is 4 bytes
            // println!("use long len");
            let mut size_be = [short_len & 0b0111_1111, 0, 0, 0];
            self.read.read_exact(&mut size_be[1..]).await?;
            len = u32::from_be_bytes(size_be) as usize;
        }

        let mut chunk = vec![0; len];

        self.read.read_exact(&mut chunk).await?;

        // debug!("read chunk: {:x?}", &chunk);

        Ok(chunk)
    }

    pub fn get_reader(self) -> Box<dyn SessionTransportReader> {
        self.read
    }

    pub fn borrow_reader(&mut self) -> &mut dyn SessionTransportReader {
        &mut self.read
    }
}

impl ChunkWriter {
    pub(crate) fn new(write: Box<dyn SessionTransportWriter>) -> Self {
        Self { write }
    }

    pub async fn write_chunk(&mut self, chunk: &[u8]) -> Result<()> {
        let len = chunk.len();
        if len > 1 << 31 {
            // error
            return Err(anyhow!("The given data chunk is to big"));
        }
        // debug!("original chunk: {:x?}", chunk);
        let len: u32 = len.try_into().unwrap();
        if len < 0b1000_0000 {
            let lenbytes = len.to_be_bytes();
            // println!("using short len: {}", lenbytes[3]);
            self.write.write_u8(lenbytes[3]).await?;
        } else {
            let lenbytes = (len | (1 << 31)).to_be_bytes();
            self.write.write_all(&lenbytes).await?;
        }

        self.write.write_all(chunk).await?;
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        self.write.shutdown().await
    }

    pub fn get_writer(self) -> Box<dyn SessionTransportWriter> {
        self.write
    }

    pub fn borrow_writer(&mut self) -> &mut dyn SessionTransportWriter {
        &mut self.write
    }
}
