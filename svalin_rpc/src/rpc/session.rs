use std::fmt::Debug;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument};

use crate::{
    permissions::PermissionHandler,
    rpc::command::handler::HandlerCollection,
    transport::{
        object_transport::{ObjectReader, ObjectReaderError, ObjectWriter, ObjectWriterError},
        session_transport::{SessionTransportReader, SessionTransportWriter},
    },
};

use super::{
    command::dispatcher::{DispatcherError, TakeableCommandDispatcher},
    peer::Peer,
};

pub struct Session {
    read: ObjectReader,
    write: ObjectWriter,
    peer: Peer,
}

impl Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").finish()
    }
}

#[derive(Serialize, Deserialize)]
pub struct SessionRequestHeader {
    pub command_key: String,
}

#[derive(Serialize, Deserialize)]
pub enum SessionResponseHeader {
    Accept,
    Decline { code: u32, message: String },
}

pub enum SessionAction {
    Error(Session, anyhow::Error),
    Closed,
    Moved,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionReadError {
    #[error("{0}")]
    ObjectReaderError(#[from] ObjectReaderError),
}

#[derive(Debug, thiserror::Error)]
pub enum SessionWriteError {
    #[error("{0}")]
    ObjectWriterError(#[from] ObjectWriterError),
}

#[derive(Debug, thiserror::Error)]
pub enum SessionDispatchError<InnerError> {
    #[error("error writing header: {0}")]
    WriteHeaderError(SessionWriteError),
    #[error("error writing request: {0}")]
    WriteRequestError(SessionWriteError),
    #[error("error reading response: {0}")]
    ReadResponseError(SessionReadError),
    #[error("session declined with code {code}: {message}")]
    SessionDeclined { code: u32, message: String },
    #[error("error running dispatcher: {0}")]
    DispatcherError(#[from] DispatcherError<InnerError>),
}

impl Session {
    pub fn new(
        read: Box<dyn SessionTransportReader>,
        write: Box<dyn SessionTransportWriter>,
        peer: Peer,
    ) -> Self {
        let read = ObjectReader::new(read);

        let write = ObjectWriter::new(write);

        Self { read, write, peer }
    }

    pub(crate) async fn handle<P>(
        mut self,
        commands: &HandlerCollection<P>,
        cancel: CancellationToken,
    ) -> Result<()>
    where
        P: PermissionHandler,
    {
        let header: SessionRequestHeader = self
            .read_object()
            .await
            .context("error reading request header")?;

        let key = header.command_key.clone();

        debug!("requested command: {key}");

        commands
            .handle_session(self, header, cancel)
            .await
            .context(format!("error handling session with key {key}"))
    }

    /// Used to send a command via this session.
    ///
    /// For this to work, the other side of the data stream needs to call
    /// `handle`. This is only meant for cascading command dispatches - in
    /// most cases you should instead use `Connection::dispatch`.
    pub async fn dispatch<D: TakeableCommandDispatcher>(
        mut self,
        dispatcher: D,
    ) -> Result<D::Output, SessionDispatchError<D::InnerError>> {
        let command_key = D::key();

        let header = SessionRequestHeader { command_key };
        self.write_object(&header)
            .await
            .map_err(SessionDispatchError::WriteHeaderError)?;

        let request = dispatcher.get_request();
        self.write_object(&request)
            .await
            .map_err(SessionDispatchError::WriteRequestError)?;

        let response: SessionResponseHeader = self
            .read_object()
            .await
            .map_err(SessionDispatchError::ReadResponseError)?;
        match response {
            SessionResponseHeader::Decline { code, message } => {
                return Err(SessionDispatchError::SessionDeclined { code, message });
            }
            SessionResponseHeader::Accept => {
                debug!("Peer accepted command: {}", D::key());
            }
        };

        let mut opt = Some(self);
        let result = dispatcher.dispatch(&mut opt, request).await;

        if let Some(session) = opt {
            session.shutdown().await;
        }

        result.map_err(SessionDispatchError::DispatcherError)
    }

    pub fn peer(&self) -> &Peer {
        &self.peer
    }

    #[instrument(skip_all)]
    pub async fn read_object<W: serde::de::DeserializeOwned>(
        &mut self,
    ) -> Result<W, SessionReadError> {
        // debug!("Reading: {}", std::any::type_name::<W>());
        Ok(self.read.read_object().await?)
    }

    #[instrument(skip_all)]
    pub async fn write_object<W: Serialize>(
        &mut self,
        object: &W,
    ) -> Result<(), SessionWriteError> {
        // debug!("Writing: {}", std::any::type_name::<W>());
        Ok(self.write.write_object(object).await?)
    }

    pub(crate) async fn shutdown(mut self) {
        debug!("Shutting down session");
        if let Err(err) = self.write.shutdown().await {
            error!("error shuting down session: {err}");
        }
    }

    pub fn destructure_transport(
        self,
    ) -> (
        Box<dyn SessionTransportReader>,
        Box<dyn SessionTransportWriter>,
        Peer,
    ) {
        (self.read.get_reader(), self.write.get_writer(), self.peer)
    }

    pub fn destructure(self) -> (ObjectReader, ObjectWriter, Peer) {
        (self.read, self.write, self.peer)
    }

    pub fn borrow_transport(
        &mut self,
    ) -> (
        &mut dyn SessionTransportReader,
        &mut dyn SessionTransportWriter,
    ) {
        (self.read.borrow_reader(), self.write.borrow_writer())
    }
}
