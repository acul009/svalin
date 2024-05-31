use anyhow::{Context, Ok, Result};
use futures::Future;
use serde::{de::DeserializeOwned, Serialize};

use super::{chunked_transport::ChunkedTransport, session_transport::SessionTransport};

pub(crate) struct ObjectTransport {
    chunked_transport: ChunkedTransport,
}

impl ObjectTransport {
    pub(crate) fn new(transport: Box<dyn SessionTransport>) -> Self {
        Self {
            chunked_transport: ChunkedTransport::new(transport),
        }
    }

    pub async fn write_object<U: Serialize>(&mut self, object: &U) -> Result<()> {
        let encoded = postcard::to_extend(&object, Vec::new())?;

        self.chunked_transport.write_chunk(&encoded).await?;

        Ok(())
    }

    pub async fn read_object<U: DeserializeOwned>(&mut self) -> Result<U> {
        let chunk = self
            .chunked_transport
            .read_chunk()
            .await
            .context("failed reading chunk")?;

        let object: U = postcard::from_bytes(&chunk).context("failed deserializing")?;

        Ok(object)
    }

    pub async fn replace_transport<R, Fut>(&mut self, replacer: R)
    where
        R: Fn(Box<dyn SessionTransport>) -> Fut,
        Fut: Future<Output = Box<dyn SessionTransport>>,
    {
        self.chunked_transport.replace_transport(replacer).await
    }

    pub async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        self.chunked_transport.shutdown().await
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
