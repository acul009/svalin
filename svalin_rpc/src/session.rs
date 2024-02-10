use std::marker::PhantomData;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    command::HandlerCollection,
    object_stream::{ObjectReader, ObjectWriter},
    session,
};

pub struct SessionCreated {}

pub struct SessionRequested {}

pub struct SessionOpen {}

pub struct SessionClosed {}

pub struct Session<T> {
    state: PhantomData<T>,
    recv: ObjectReader,
    send: ObjectWriter,
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

impl Session<_> {
    pub(crate) fn new(
        recv: Box<dyn AsyncRead + Send + Unpin>,
        send: Box<dyn AsyncWrite + Send + Unpin>,
    ) -> Session<SessionCreated> {
        Session {
            state: PhantomData,
            recv: ObjectReader::new(recv),
            send: ObjectWriter::new(send),
        }
    }
}

impl Session<SessionCreated> {
    fn open(self) -> Session<SessionOpen> {
        Session {
            state: PhantomData,
            recv: self.recv,
            send: self.send,
        }
    }

    async fn receive_header(self) -> Result<(Session<SessionOpen>, SessionRequestHeader)> {
        let mut session = self.open();

        let header: SessionRequestHeader = session.read_object().await?;

        Ok((session, header))
    }

    pub(crate) async fn handle(self, commands: &HandlerCollection) -> Result<()> {
        let (mut session, header) = self.receive_header().await?;

        if let Some(mut command) = commands.get(&header.command_key) {
            let response = SessionResponseHeader::Accept(SessionAcceptedHeader {});
            session.write_object(&response).await?;

            command.handle(session).await?;
        } else {
            let response = SessionResponseHeader::Decline(SessionDeclinedHeader {
                code: 404,
                message: "command not found".to_string(),
            });
            session.write_object(&response).await?;
        }

        todo!()
    }
}

impl Session<SessionOpen> {
    pub async fn read_object<W: serde::de::DeserializeOwned>(&mut self) -> Result<W> {
        self.recv.read_object().await
    }

    pub async fn write_object<W: Serialize>(&mut self, object: &W) -> Result<()> {
        self.send.write_object(object).await
    }
}
