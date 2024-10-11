use std::fmt::Debug;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, instrument};

use crate::{
    permissions::{PermissionCheckError, PermissionHandler},
    rpc::command::handler::HandlerCollection,
    transport::{
        object_transport::{ObjectReader, ObjectWriter},
        session_transport::{SessionTransportReader, SessionTransportWriter},
    },
};

use super::{command::dispatcher::TakeableCommandDispatcher, peer::Peer};

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

pub enum SessionAction {
    Error(Session, anyhow::Error),
    Closed,
    Moved,
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

    pub(crate) async fn handle(mut self, commands: &HandlerCollection) -> Result<()> {
        debug!("waiting for request header");

        let header: SessionRequestHeader = self.read_object().await?;

        debug!("requested command: {}", header.command_key);

        if let Some(command) = commands.get(&header.command_key).await {
            let response = SessionResponseHeader::Accept(SessionAcceptedHeader {});
            self.write_object(&response).await?;

            let mut session = Some(self);

            command.handle(&mut session).await?;

            if let Some(session) = session {
                session.shutdown().await;
            }

            // todo
        } else {
            let response = SessionResponseHeader::Decline(SessionDeclinedHeader {
                code: 404,
                message: "command not found".to_string(),
            });
            self.write_object(&response).await?;
            self.shutdown().await;
        }

        Ok(())
    }

    /// Used to send a command via this session.
    ///
    /// For this to work, the other side of the data stream needs to call
    /// `handle`. This is only meant for cascading command dispatches - in
    /// most cases you should instead use `Connection::dispatch`.
    pub async fn dispatch<D: TakeableCommandDispatcher>(
        mut self,
        dispatcher: D,
    ) -> Result<D::Output> {
        let command_key = dispatcher.key();

        self.request_session(command_key).await?;

        let mut opt = Some(self);

        let result = dispatcher.dispatch(&mut opt).await;

        if let Some(session) = opt {
            session.shutdown().await;
        }

        result
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

    pub(crate) async fn shutdown(mut self) {
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

pub struct UnauthorizedSession {
    session: Session,
}

impl UnauthorizedSession {
    pub async fn authorize<'a, H: PermissionHandler>(
        &'a mut self,
        permission_handler: &H,
        permission: &H::Permission,
    ) -> Result<&'a mut Session, PermissionCheckError> {
        let perm_check = permission_handler.may(&self.session.peer, permission).await;

        match perm_check {
            Ok(_) => {
                self.session.write_object::<Result<(), ()>>(&Ok(())).await?;
                Ok(&mut self.session)
            }
            Err(err) => {
                // Todo: handle error somehow?
                let _err = self.session.write_object::<Result<(), ()>>(&Err(())).await;

                Err(err)
            }
        }
    }

    pub async fn read_object<W: serde::de::DeserializeOwned>(&mut self) -> Result<W> {
        // debug!("Reading: {}", std::any::type_name::<W>());
        self.session.read_object().await
    }
}

pub struct PendingSession {
    session: Session,
}

impl PendingSession {
    pub async fn check_authorization<'a>(&'a mut self) -> Result<&'a mut Session> {
        let perm_result: Result<(), ()> = self.session.read_object().await?;

        match perm_result {
            Ok(_) => Ok(&mut self.session),
            Err(_) => Err(anyhow!("Rpc-Error: Permission denied")),
        }
    }

    pub async fn write_object<W: Serialize>(&mut self, object: &W) -> Result<()> {
        self.session.write_object(object).await
    }
}
