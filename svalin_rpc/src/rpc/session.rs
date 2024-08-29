use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use anyhow::{anyhow, Result};
use futures::Future;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, instrument};

use crate::{
    rpc::command::HandlerCollection,
    transport::{
        chunked_transport::{ChunkReader, ChunkWriter},
        dummy_transport::{DummyTransport, DummyTransportReader, DummyTransportWriter},
        object_transport::{ObjectReader, ObjectTransport, ObjectWriter},
        session_transport::{SessionTransport, SessionTransportReader, SessionTransportWriter},
    },
};

use super::peer::Peer;

pub struct Session {
    read: ObjectReader,
    write: ObjectWriter,
    partner: Peer,
}

impl Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").finish()
    }
}

#[derive(Serialize, Deserialize)]
struct SessionRequestHeader {
    command_key: String,
}

#[derive(Serialize, Deserialize)]
enum SessionResponseHeader {
    Accept(SessionAcceptedHeader),
    Decline(SessionDeclinedHeader),
}

#[derive(Serialize, Deserialize)]
struct SessionAcceptedHeader {}

#[derive(Serialize, Deserialize)]
struct SessionDeclinedHeader {
    code: u32,
    message: String,
}

pub struct SessionClosed {}

impl Session {
    pub(crate) fn new(
        read: Box<dyn SessionTransportReader>,
        write: Box<dyn SessionTransportWriter>,
    ) -> Self {
        let read = ObjectReader::new(ChunkReader::new(read));

        let write = ObjectWriter::new(ChunkWriter::new(write));

        Self {
            read,
            write,
            partner: Peer::Anonymous,
        }
    }

    pub fn dangerous_create_dummy_session() -> Self {
        Self::new(
            Box::new(DummyTransportReader::default()),
            Box::new(DummyTransportWriter::default()),
        )
    }

    pub(crate) async fn handle(&mut self, commands: HandlerCollection) -> Result<()> {
        debug!("waiting for request header");

        let header: SessionRequestHeader = self.read_object().await?;

        debug!("requested command: {}", header.command_key);

        if let Some(command) = commands.get(&header.command_key).await {
            let response = SessionResponseHeader::Accept(SessionAcceptedHeader {});
            self.write_object(&response).await?;

            command.handle(self).await?;

            // todo
        } else {
            let response = SessionResponseHeader::Decline(SessionDeclinedHeader {
                code: 404,
                message: "command not found".to_string(),
            });
            self.write_object(&response).await?;
        }

        self.shutdown().await?;

        Ok(())
    }

    pub(crate) async fn request_session(&mut self, command_key: String) -> Result<()> {
        let header = SessionRequestHeader { command_key };

        self.write_object(&header).await?;

        let response: SessionResponseHeader = self.read_object().await?;
        match response {
            SessionResponseHeader::Decline(declined) => {
                Err(anyhow!(format!("{}: {}", declined.code, declined.message)))
            }
            SessionResponseHeader::Accept(_accepted) => Ok(()),
        }
    }

    #[instrument(skip_all)]
    pub async fn read_object<W: serde::de::DeserializeOwned>(&mut self) -> Result<W> {
        // debug!("Reading: {}", std::any::type_name::<W>());
        self.read.read_object().await
    }

    #[instrument(skip_all)]
    pub async fn write_object<W: Serialize>(&mut self, object: &W) -> Result<()> {
        // debug!("Writing: {}", std::any::type_name::<W>());
        self.write.write_object(object).await
    }

    pub(crate) async fn shutdown(mut self) -> SessionClosed {
        if let Err(err) = self.write.shutdown().await {
            error!("error shuting down session: {err}");
        }

        SessionClosed {}
    }

    // pub async fn forward_session(&mut self, partner: &mut Self) -> Result<()> {
    //     self.forward_transport(partner.transport.borrow_transport())
    //         .await?;

    //     Ok(())
    // }

    // pub(crate) async fn forward_transport(
    //     &mut self,
    //     transport: &mut Box<dyn SessionTransport>,
    // ) -> Result<()> {
    //     debug!("starting bidirectional copy");

    //     tokio::io::copy_bidirectional(self.transport.borrow_transport(),
    // transport).await?;

    //     debug!("finished bidirectional copy");

    //     Ok(())
    // }

    // pub fn extract_transport(self) -> Box<dyn SessionTransport> {
    //     self.transport.extract_transport()
    // }
}
