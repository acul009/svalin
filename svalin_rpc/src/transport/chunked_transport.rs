use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::transport::session_transport::SessionTransport;

pub struct ChunkTransport {
    transport: Box<dyn SessionTransport>,
    read_chunk: Option<usize>,
    read_buffer: Vec<u8>,
}

impl ChunkTransport {
    pub fn new(transport: Box<dyn SessionTransport>) -> Self {
        Self {
            transport,
            read_chunk: None,
            read_buffer: Vec::new(),
        }
    }

    pub async fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), ChunkWriterError> {
        let len = ChunkLength::from_usize(chunk.len());

        self.transport
            .write_all(len.as_bytes())
            .await
            .map_err(|err| ChunkWriterError::ExtendedLengthWriteError(err))?;

        self.transport
            .write_all(chunk)
            .await
            .map_err(|err| ChunkWriterError::BodyWriteError(err))?;

        Ok(())
    }

    pub async fn read_chunk(&mut self) -> Result<Vec<u8>, ChunkReaderError> {
        let read_chunk = match self.read_chunk {
            None => {
                // Todo: fix non cancel safe read
                let short_len = self
                    .transport
                    .read_u8()
                    .await
                    .map_err(|err| ChunkReaderError::LengthReadError(err))?;

                // println!("read short len: {}", short_len);

                let len = match ChunkLength::try_from_byte(short_len) {
                    Some(len) => len,
                    None => {
                        let mut size = [short_len, 0, 0, 0];
                        self.transport
                            .read_exact(&mut size[1..])
                            .await
                            .map_err(|err| ChunkReaderError::LengthReadError(err))?;
                        ChunkLength::from_4bytes(size)
                    }
                };

                let len = len.to_usize();
                self.read_chunk = Some(len);
                len
            }
            Some(len) => len,
        };

        while self.read_buffer.len() < read_chunk {
            let read = self
                .transport
                .read_buf(&mut self.read_buffer)
                .await
                .map_err(|err| ChunkReaderError::BodyReadError(err))?;
            if read == 0 {
                return Err(ChunkReaderError::UnexpectedEndOfStream);
            }
        }

        let mut new_buffer = self.read_buffer.split_off(read_chunk);
        std::mem::swap(&mut new_buffer, &mut self.read_buffer);
        self.read_chunk = None;

        // tracing::trace!("read chunk: {:x?}", &chunk);

        Ok(new_buffer)
    }

    pub async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        self.transport.shutdown().await
    }

    pub fn borrow_transport(&mut self) -> &mut dyn SessionTransport {
        &mut self.transport
    }

    pub fn into_transport(self) -> Box<dyn SessionTransport> {
        self.transport
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ChunkReaderError {
    #[error("Failed to read chunk length: {0}")]
    LengthReadError(std::io::Error),
    #[error("Failed to read extended chunk length: {0}")]
    ExtendedLengthReadError(std::io::Error),
    #[error("Failed to read chunk body: {0}")]
    BodyReadError(std::io::Error),
    #[error("Unexpected end of stream")]
    UnexpectedEndOfStream,
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
        // tracing::trace!("original chunk: {:x?}", chunk);
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
