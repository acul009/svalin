use anyhow::{Context, Ok, Result};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use self::chunked_stream::{ChunkReader, ChunkWriter};

mod chunked_stream;

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

        println!("writing chunk: {:?}", encoded);
        
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
