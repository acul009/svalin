use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::session_transport::{SessionTransportReader, SessionTransportWriter};

pub(crate) struct ChunkReader {
    read: Box<dyn SessionTransportReader>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChunkReaderError {
    #[error("Failed to read chunk length: {0}")]
    LengthReadError(std::io::Error),
    #[error("Failed to read extended chunk length: {0}")]
    ExtendedLengthReadError(std::io::Error),
    #[error("Failed to read chunk body: {0}")]
    BodyReadError(std::io::Error),
}

impl ChunkReader {
    pub(crate) fn new(read: Box<dyn SessionTransportReader>) -> Self {
        Self { read }
    }

    pub async fn read_chunk(&mut self) -> Result<Vec<u8>, ChunkReaderError> {
        let short_len = self
            .read
            .read_u8()
            .await
            .map_err(|err| ChunkReaderError::LengthReadError(err))?;

        let len: usize;

        // println!("read short len: {}", short_len);

        if short_len < 0b1000_0000 {
            len = short_len.into()
        } else {
            // length is 4 bytes
            // println!("use long len");
            let mut size_be = [short_len & 0b0111_1111, 0, 0, 0];
            self.read
                .read_exact(&mut size_be[1..])
                .await
                .map_err(|err| ChunkReaderError::LengthReadError(err))?;
            len = u32::from_be_bytes(size_be) as usize;
        }

        let mut chunk = vec![0; len];

        self.read
            .read_exact(&mut chunk)
            .await
            .map_err(|err| ChunkReaderError::BodyReadError(err))?;

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

pub(crate) struct ChunkWriter {
    write: Box<dyn SessionTransportWriter>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChunkWriterError {
    #[error("The given data chunk is to big")]
    ChunkTooBig,
    #[error("Failed to write chunk length: {0}")]
    LengthWriteError(std::io::Error),
    #[error("Failed to write extended chunk length: {0}")]
    ExtendedLengthWriteError(std::io::Error),
    #[error("Failed to write chunk body: {0}")]
    BodyWriteError(std::io::Error),
}

impl ChunkWriter {
    pub(crate) fn new(write: Box<dyn SessionTransportWriter>) -> Self {
        Self { write }
    }

    pub async fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), ChunkWriterError> {
        let len = chunk.len();
        if len > 1 << 31 {
            return Err(ChunkWriterError::ChunkTooBig);
        }
        // debug!("original chunk: {:x?}", chunk);
        let len: u32 = len.try_into().unwrap();
        if len < 0b1000_0000 {
            let lenbytes = len.to_be_bytes();
            // println!("using short len: {}", lenbytes[3]);
            self.write
                .write_u8(lenbytes[3])
                .await
                .map_err(|err| ChunkWriterError::LengthWriteError(err))?;
        } else {
            let lenbytes = (len | (1 << 31)).to_be_bytes();
            self.write
                .write_all(&lenbytes)
                .await
                .map_err(|err| ChunkWriterError::ExtendedLengthWriteError(err))?;
        }

        self.write
            .write_all(chunk)
            .await
            .map_err(|err| ChunkWriterError::BodyWriteError(err))?;

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
