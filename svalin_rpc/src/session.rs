use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::{
    command::HandlerCollection,
    transport::{object_transport::ObjectTransport, session_transport::SessionTransport},
};

pub struct SessionCreated;

pub struct SessionOpen;

pub struct Session<T> {
    state: PhantomData<T>,
    transport: ObjectTransport,
}

impl<T> Debug for Session<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("state", &self.state)
            .finish()
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

impl Session<()> {
    pub(crate) fn new(transport: Box<dyn SessionTransport>) -> Session<SessionCreated> {
        Session {
            state: PhantomData,
            transport: ObjectTransport::new(transport),
        }
    }
}

impl Session<SessionCreated> {
    fn open(self) -> Session<SessionOpen> {
        Session {
            state: PhantomData,
            transport: self.transport,
        }
    }

    async fn receive_header(self) -> Result<(Session<SessionOpen>, SessionRequestHeader)> {
        let mut session = self.open();

        let header: SessionRequestHeader = session.read_object().await?;

        Ok((session, header))
    }

    pub(crate) async fn request_session(self, command_key: String) -> Result<Session<SessionOpen>> {
        let header = SessionRequestHeader { command_key };

        let mut session = self.open();
        session.write_object(&header).await?;

        let response: SessionResponseHeader = session.read_object().await?;
        match response {
            SessionResponseHeader::Decline(declined) => {
                Err(anyhow!(format!("{}: {}", declined.code, declined.message)))
            }
            SessionResponseHeader::Accept(_accepted) => Ok(session),
        }
    }

    pub(crate) async fn handle(self, commands: Arc<HandlerCollection>) -> Result<()> {
        let (mut session, header) = self.receive_header().await?;

        debug!("requested command: {}", header.command_key);

        if let Some(command) = commands.get(&header.command_key).await {
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

        // not sure if I need a todo here?

        Ok(())
    }
}

impl Session<SessionOpen> {
    #[instrument]
    pub async fn read_object<W: serde::de::DeserializeOwned>(&mut self) -> Result<W> {
        self.transport.read_object().await
    }

    #[instrument(skip_all)]
    pub async fn write_object<W: Serialize>(&mut self, object: &W) -> Result<()> {
        self.transport.write_object(object).await
    }
}
