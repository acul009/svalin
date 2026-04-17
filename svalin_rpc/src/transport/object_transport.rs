use serde::{Serialize, de::DeserializeOwned};

use crate::transport::{chunked_transport::ChunkTransport, session_transport::SessionTransport};

use super::chunked_transport::{ChunkReaderError, ChunkWriterError};

pub struct ObjectTransport {
    transport: ChunkTransport,
}

impl ObjectTransport {
    pub fn new(transport: Box<dyn SessionTransport>) -> Self {
        Self {
            transport: ChunkTransport::new(transport),
        }
    }

    pub async fn write_object<U: Serialize>(
        &mut self,
        object: &U,
    ) -> Result<(), ObjectWriterError> {
        let encoded = postcard::to_extend(object, Vec::new())?;

        #[cfg(test)]
        {
            // sending the type if for easier test debugging
            let type_name = std::any::type_name::<U>();
            self.transport.write_chunk(type_name.as_bytes()).await?;
        }
        self.transport.write_chunk(&encoded).await?;

        Ok(())
    }

    pub async fn read_object<U: DeserializeOwned>(&mut self) -> Result<U, ObjectReaderError> {
        #[cfg(test)]
        {
            // reading and comparing the type if for easier test debugging
            let chunk = self.transport.read_chunk().await?;
            let sent_type = String::from_utf8_lossy(&chunk);
            let type_name = std::any::type_name::<U>();
            if sent_type != type_name {
                panic!("expected type: {}, got: {}", type_name, sent_type);
            }
        }

        let chunk = self.transport.read_chunk().await?;

        let object: U = postcard::from_bytes(&chunk)?;

        Ok(object)
    }

    pub async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        self.transport.shutdown().await
    }

    pub fn borrow_transport(&mut self) -> &mut dyn SessionTransport {
        self.transport.borrow_transport()
    }

    pub fn into_transport(self) -> Box<dyn SessionTransport> {
        self.transport.into_transport()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ObjectReaderError {
    #[error("Failed to read chunk: {0}")]
    ChunkReadError(#[from] ChunkReaderError),
    #[error("Failed to deserialize object: {0}")]
    DeserializeError(#[from] postcard::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ObjectWriterError {
    #[error("Failed to write chunk: {0}")]
    ChunkWriteError(#[from] ChunkWriterError),
    #[error("Failed to serialize object: {0}")]
    SerializeError(#[from] postcard::Error),
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
