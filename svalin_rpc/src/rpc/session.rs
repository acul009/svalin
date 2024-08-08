use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use anyhow::{anyhow, Result};
use futures::Future;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::{
    rpc::command::HandlerCollection,
    transport::{
        dummy_transport::DummyTransport, object_transport::ObjectTransport,
        session_transport::SessionTransport,
    },
};

use super::peer::Peer;

pub struct SessionCreated;

pub struct SessionOpen;

pub struct Session<T> {
    state: PhantomData<T>,
    transport: ObjectTransport,
    partner: Peer,
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
            partner: Peer::Anonymous,
        }
    }

    pub fn dangerous_create_dummy_session() -> Session<SessionOpen> {
        Session {
            state: PhantomData,
            transport: ObjectTransport::new(Box::new(DummyTransport::new())),
            partner: Peer::Anonymous,
        }
    }
}

impl Session<SessionCreated> {
    fn open(self) -> Session<SessionOpen> {
        Session {
            state: PhantomData,
            transport: self.transport,
            partner: self.partner,
        }
    }

    pub(crate) async fn handle(self, commands: HandlerCollection) -> Result<()> {
        self.open().handle(commands).await
    }

    pub(crate) async fn request_session(self, command_key: String) -> Result<Session<SessionOpen>> {
        let mut open = self.open();
        match open.request_session(command_key).await {
            Ok(_) => Ok(open),
            Err(err) => Err(err),
        }
    }
}

impl Session<SessionOpen> {
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
        self.transport.read_object().await
    }

    #[instrument(skip_all)]
    pub async fn write_object<W: Serialize>(&mut self, object: &W) -> Result<()> {
        // debug!("Writing: {}", std::any::type_name::<W>());
        self.transport.write_object(object).await
    }

    pub async fn replace_transport<R, Fut>(&mut self, replacer: R)
    where
        R: FnOnce(Box<dyn SessionTransport>) -> Fut,
        Fut: Future<Output = Box<dyn SessionTransport>>,
    {
        self.transport.replace_transport(replacer).await
    }

    pub(crate) async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        self.transport.shutdown().await
    }

    pub async fn forward_session(&mut self, partner: &mut Self) -> Result<()> {
        self.forward_transport(partner.transport.borrow_transport())
            .await?;

        Ok(())
    }

    pub(crate) async fn forward_transport(
        &mut self,
        transport: &mut Box<dyn SessionTransport>,
    ) -> Result<()> {
        debug!("starting bidirectional copy");

        tokio::io::copy_bidirectional(self.transport.borrow_transport(), transport).await?;

        debug!("finished bidirectional copy");

        Ok(())
    }

    pub fn extract_transport(self) -> Box<dyn SessionTransport> {
        self.transport.extract_transport()
    }
}
