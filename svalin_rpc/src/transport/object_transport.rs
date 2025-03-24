use serde::{Serialize, de::DeserializeOwned};

use super::{
    chunked_transport::{ChunkReader, ChunkReaderError, ChunkWriter, ChunkWriterError},
    session_transport::{SessionTransportReader, SessionTransportWriter},
};

pub struct ObjectReader {
    read: ChunkReader,
}

#[derive(Debug, thiserror::Error)]
pub enum ObjectReaderError {
    #[error("Failed to read chunk: {0}")]
    ChunkReadError(#[from] ChunkReaderError),
    #[error("Failed to deserialize object: {0}")]
    DeserializeError(#[from] postcard::Error),
}

impl ObjectReader {
    pub(crate) fn new(read: Box<dyn SessionTransportReader>) -> Self {
        Self {
            read: ChunkReader::new(read),
        }
    }

    pub async fn read_object<U: DeserializeOwned>(&mut self) -> Result<U, ObjectReaderError> {
        let chunk = self.read.read_chunk().await?;

        let object: U = postcard::from_bytes(&chunk)?;

        Ok(object)
    }

    pub fn get_reader(self) -> Box<dyn SessionTransportReader> {
        self.read.get_reader()
    }

    pub fn borrow_reader(&mut self) -> &mut dyn SessionTransportReader {
        self.read.borrow_reader()
    }
}

pub struct ObjectWriter {
    write: ChunkWriter,
}

#[derive(Debug, thiserror::Error)]
pub enum ObjectWriterError {
    #[error("Failed to write chunk: {0}")]
    ChunkWriteError(#[from] ChunkWriterError),
    #[error("Failed to serialize object: {0}")]
    SerializeError(#[from] postcard::Error),
}

impl ObjectWriter {
    pub(crate) fn new(write: Box<dyn SessionTransportWriter>) -> Self {
        Self {
            write: ChunkWriter::new(write),
        }
    }

    pub async fn write_object<U: Serialize>(
        &mut self,
        object: &U,
    ) -> Result<(), ObjectWriterError> {
        let encoded = postcard::to_extend(&object, Vec::new())?;

        self.write.write_chunk(&encoded).await?;

        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        self.write.shutdown().await
    }

    pub fn get_writer(self) -> Box<dyn SessionTransportWriter> {
        self.write.get_writer()
    }

    pub fn borrow_writer(&mut self) -> &mut dyn SessionTransportWriter {
        self.write.borrow_writer()
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
