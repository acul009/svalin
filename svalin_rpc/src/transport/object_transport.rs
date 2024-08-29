use anyhow::{Context, Ok, Result};
use serde::{de::DeserializeOwned, Serialize};

use super::chunked_transport::{ChunkReader, ChunkWriter};

pub(crate) struct ObjectReader {
    read: ChunkReader,
}

pub(crate) struct ObjectWriter {
    write: ChunkWriter,
}

impl ObjectReader {
    pub fn new(read: ChunkReader) -> Self {
        Self { read }
    }

    pub async fn read_object<U: DeserializeOwned>(&mut self) -> Result<U> {
        let chunk = self
            .read
            .read_chunk()
            .await
            .context("failed reading chunk")?;

        let object: U = postcard::from_bytes(&chunk).context("failed deserializing")?;

        Ok(object)
    }
}

impl ObjectWriter {
    pub fn new(write: ChunkWriter) -> Self {
        Self { write }
    }

    pub async fn write_object<U: Serialize>(&mut self, object: &U) -> Result<()> {
        let encoded = postcard::to_extend(&object, Vec::new())?;

        self.write.write_chunk(&encoded).await?;

        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        self.write.shutdown().await
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
