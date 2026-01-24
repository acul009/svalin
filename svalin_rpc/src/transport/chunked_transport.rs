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

        // println!("read short len: {}", short_len);

        let len = match ChunkLength::try_from_byte(short_len) {
            Some(len) => len,
            None => {
                let mut size = [short_len, 0, 0, 0];
                self.read
                    .read_exact(&mut size[1..])
                    .await
                    .map_err(|err| ChunkReaderError::LengthReadError(err))?;
                ChunkLength::from_4bytes(size)
            }
        };

        let mut chunk = vec![0; len.to_usize()];

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
        let len = ChunkLength::from_usize(chunk.len());

        self.write
            .write_all(len.as_bytes())
            .await
            .map_err(|err| ChunkWriterError::ExtendedLengthWriteError(err))?;

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

pub enum ChunkLength {
    Byte(u8),
    U32([u8; 4]),
}

impl ChunkLength {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Byte(b) => std::slice::from_ref(b),
            Self::U32(b) => b,
        }
    }

    pub fn from_usize(len: usize) -> ChunkLength {
        if len > 1 << 31 {
            panic!("len too big")
        }
        // debug!("original chunk: {:x?}", chunk);
        let len: u32 = len.try_into().unwrap();
        if len < 0b1000_0000 {
            let lenbytes = len.to_be_bytes();
            Self::Byte(lenbytes[3])
        } else {
            let lenbytes = (len | (1 << 31)).to_be_bytes();
            Self::U32(lenbytes)
        }
    }

    pub fn try_from_byte(byte: u8) -> Option<Self> {
        if byte < 0b1000_0000 {
            Some(Self::Byte(byte))
        } else {
            None
        }
    }

    pub fn from_4bytes(bytes: [u8; 4]) -> Self {
        Self::U32(bytes)
    }

    pub fn to_usize(&self) -> usize {
        match self {
            Self::Byte(b) => *b as usize,
            Self::U32(b) => {
                let mut b = *b;
                b[0] &= 0b0111_1111;
                u32::from_be_bytes(b) as usize
            }
        }
    }
}
