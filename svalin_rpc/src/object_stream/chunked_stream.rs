use anyhow::{Ok, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub(super) struct ChunkWriter {
    binary_writer: Box<dyn AsyncWrite + Send + Unpin>,
}

impl ChunkWriter {
    pub(super) fn new(binary_writer: Box<dyn AsyncWrite + Send + Unpin>) -> Self {
        Self { binary_writer }
    }

    pub async fn write_chunk(&mut self, chunk: &[u8]) -> Result<()> {
        let len = chunk.len();
        if len > 1 << 31 {
            // error
            todo!()
        }
        let len: u32 = len.try_into().unwrap();
        if len >= 0b1000_0000 {
            let lenbytes = (len | (1 << 31)).to_be_bytes();
            self.binary_writer.write_all(&lenbytes).await?;
        } else {
            let lenbytes = len.to_be_bytes();
            self.binary_writer.write_u8(lenbytes[3]).await?;
        }

        self.binary_writer.write_all(chunk).await?;
        Ok(())
    }
}

pub(super) struct ChunkReader {
    binary_reader: Box<dyn AsyncRead + Send + Unpin>,
}

impl ChunkReader {
    pub(super) fn new(binary_reader: Box<dyn AsyncRead + Send + Unpin>) -> Self {
        Self { binary_reader }
    }

    pub async fn read_chunk(&mut self) -> Result<Vec<u8>> {
        let short_len = self.binary_reader.read_u8().await?;

        let len: usize;

        if short_len >= 0b1000_0000 {
            len = short_len.into()
        } else {
            // length is 4 bytes
            let mut size_be = [short_len & 0b0111_1111, 0, 0, 0];
            self.binary_reader.read_exact(&mut size_be[1..]).await?;
            len = u32::from_be_bytes(size_be) as usize;
        }

        let mut chunk = Vec::with_capacity(len);
        self.binary_reader.read_exact(&mut chunk).await?;

        Ok(chunk)
    }
}
