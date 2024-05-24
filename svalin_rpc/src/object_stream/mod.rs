use anyhow::{Context, Ok, Result};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use self::{
    chunked_stream::{ChunkReader, ChunkWriter, ChunkedTransport},
    session_transport::SessionTransport,
};

mod chunked_stream;
pub mod session_transport;

pub(crate) struct ObjectWriter {
    chunk_writer: ChunkWriter,
}

impl ObjectWriter {
    pub(super) fn new(binary_writer: Box<dyn AsyncWrite + Send + Unpin>) -> Self {
        let chunk_writer = chunked_stream::ChunkWriter::new(binary_writer);
        Self { chunk_writer }
    }

    pub(super) async fn write_object<U: Serialize>(&mut self, object: &U) -> Result<()> {
        let encoded = postcard::to_extend(&object, Vec::new())?;

        self.chunk_writer.write_chunk(&encoded).await?;

        Ok(())
    }
}

pub(crate) struct ObjectReader {
    chunk_reader: ChunkReader,
}

impl ObjectReader {
    pub(super) fn new(binary_reader: Box<dyn AsyncRead + Send + Unpin>) -> Self {
        let chunk_reader = chunked_stream::ChunkReader::new(binary_reader);
        Self { chunk_reader }
    }
    pub(super) async fn read_object<U: DeserializeOwned>(&mut self) -> Result<U> {
        let chunk = self
            .chunk_reader
            .read_chunk()
            .await
            .context("failed reading chunk")?;

        let object: U = postcard::from_bytes(&chunk).context("failed deserializing")?;

        Ok(object)
    }
}

pub(crate) struct ObjectTransport {
    chunked_transport: ChunkedTransport,
}

impl ObjectTransport {
    pub(super) fn new(transport: Box<dyn SessionTransport>) -> Self {
        Self {
            chunked_transport: ChunkedTransport::new(transport),
        }
    }

    pub(super) async fn write_object<U: Serialize>(&mut self, object: &U) -> Result<()> {
        let encoded = postcard::to_extend(&object, Vec::new())?;

        self.chunked_transport.write_chunk(&encoded).await?;

        Ok(())
    }

    pub(super) async fn read_object<U: DeserializeOwned>(&mut self) -> Result<U> {
        let chunk = self
            .chunked_transport
            .read_chunk()
            .await
            .context("failed reading chunk")?;

        let object: U = postcard::from_bytes(&chunk).context("failed deserializing")?;

        Ok(object)
    }
}

#[cfg(test)]
mod test {
    use std::time::SystemTime;

    #[test]
    fn test_postcard_u64() {
        let number = 98345254575894875u64;
        let encoded = postcard::to_extend(&number, Vec::new()).unwrap();
        let copy: u64 = postcard::from_bytes(&encoded).unwrap();
        assert!(number == copy, "postcard u64 test failed");
    }

    #[test]
    fn test_system_time() {
        let now = SystemTime::now();
        let encoded = postcard::to_extend(&now, Vec::new()).unwrap();
        let copy: SystemTime = postcard::from_bytes(&encoded).unwrap();
        assert!(now == copy, "postcard system time test failed")
    }
}
