use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::transport::session_transport::SessionTransport;

pub struct ChunkTransport {
    transport: Box<dyn SessionTransport>,
    read_buffer: Vec<u8>,
    read_buffer_size: usize,
}

impl ChunkTransport {
    pub fn new(transport: Box<dyn SessionTransport>) -> Self {
        Self {
            transport,
            read_buffer: vec![0; 1024],
            read_buffer_size: 0,
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
        // tracing::trace!("entering read_chunk");
        let read_chunk = {
            if self.read_buffer_size == 0 {
                let short_len = self
                    .transport
                    .read_u8()
                    .await
                    .map_err(|err| ChunkReaderError::LengthReadError(err))?;
                self.read_buffer[0] = short_len;
                self.read_buffer_size += 1;
            }

            let len = match ChunkLength::try_from_byte(self.read_buffer[0]) {
                Some(len) => len,
                None => {
                    while self.read_buffer_size < 4 {
                        // tracing::trace!("reading {}th byte", self.read_buffer_size);
                        let short_len = self
                            .transport
                            .read_u8()
                            .await
                            .map_err(|err| ChunkReaderError::LengthReadError(err))?;
                        self.read_buffer[self.read_buffer_size] = short_len;
                        self.read_buffer_size += 1;
                    }
                    let size: &[u8; 4] = self
                        .read_buffer
                        .first_chunk()
                        .expect("already ensured the correct size");
                    ChunkLength::from_4bytes(size.clone())
                }
            };

            len
        };

        let header_len = read_chunk.byte_len();
        let payload_len = read_chunk.to_usize();
        let total_len = header_len + payload_len;

        while self.read_buffer.len() < total_len {
            self.read_buffer.push(0);
        }

        while self.read_buffer_size < total_len {
            let read = self
                .transport
                .read(&mut self.read_buffer[self.read_buffer_size..total_len])
                .await
                .map_err(|err| ChunkReaderError::BodyReadError(err))?;
            // tracing::trace!("read {} bytes", read);
            self.read_buffer_size += read;
            // tracing::trace!(
            //     "bytes in buffer: {} of {}",
            //     self.read_buffer_size,
            //     total_len
            // );
            if read == 0 {
                return Err(ChunkReaderError::UnexpectedEndOfStream);
            }
        }

        let chunk = self.read_buffer[header_len..total_len].to_vec();
        self.read_buffer_size = 0;

        // tracing::trace!("exiting read_chunk");
        Ok(chunk)
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

    fn byte_len(&self) -> usize {
        match self {
            Self::Byte(_) => 1,
            Self::U32(_) => 4,
        }
    }
}
