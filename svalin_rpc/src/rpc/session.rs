use std::fmt::Debug;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, instrument};

use crate::{
    permissions::PermissionHandler,
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

    pub(crate) async fn handle<P>(mut self, commands: &HandlerCollection<P>) -> Result<()>
    where
        P: PermissionHandler,
    {
        debug!("waiting for request header");

        let header: SessionRequestHeader = self
            .read_object()
            .await
            .context("error reading request header")?;

        let key = header.command_key.clone();

        debug!("requested command: {key}");

        commands
            .handle_session(self, header)
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
    ) -> Result<D::Output> {
        let command_key = D::key();

        let header = SessionRequestHeader { command_key };
        self.write_object(&header).await?;

        let request = dispatcher.get_request();
        self.write_object(&request).await?;

        let response: SessionResponseHeader = self.read_object().await?;
        match response {
            SessionResponseHeader::Decline { code, message } => {
                return Err(anyhow!(format!("Error Code {}: {}", code, message)));
            }
            SessionResponseHeader::Accept => {
                debug!("Peer accepted command: {}", D::key());
            }
        };

        let mut opt = Some(self);
        let result = dispatcher
            .dispatch(&mut opt, request)
            .await
            .context("error while dispatcher was running");

        if let Some(session) = opt {
            session.shutdown().await;
        }

        result
    }

    pub fn peer(&self) -> &Peer {
        &self.peer
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

// pub struct UnauthorizedSession<P> {
//     session: Session,
//     permission_handler: P,
// }

// impl<P> UnauthorizedSession<P>
// where
//     P: PermissionHandler,
// {
//     pub async fn authorize<'a>(
//         &'a mut self,
//         permission: impl Into<P::Permission> + Sized,
//     ) -> Result<&'a mut Session, PermissionCheckError> {
//         let perm_check = self
//             .permission_handler
//             .may(&self.session.peer, permission)
//             .await;

//         match perm_check {
//             Ok(_) => {
//                 self.session.write_object::<Result<(), ()>>(&Ok(())).await?;
//                 Ok(&mut self.session)
//             }
//             Err(err) => {
//                 // Todo: handle error somehow?
//                 let _err = self.session.write_object::<Result<(),
// ()>>(&Err(())).await;

//                 Err(err)
//             }
//         }
//     }

//     pub async fn authorize_owned<'a, H: PermissionHandler>(
//         mut self,
//         permission: &P::Permission,
//     ) -> Result<Session, PermissionCheckError> {
//         let perm_check = self
//             .permission_handler
//             .may(&self.session.peer, permission)
//             .await;

//         match perm_check {
//             Ok(_) => {
//                 self.session.write_object::<Result<(), ()>>(&Ok(())).await?;
//                 Ok(self.session)
//             }
//             Err(err) => {
//                 // Todo: handle error somehow?
//                 let _err = self.session.write_object::<Result<(),
// ()>>(&Err(())).await;

//                 Err(err)
//             }
//         }
//     }

//     pub async fn read_object<W: serde::de::DeserializeOwned>(&mut self) ->
// Result<W> {         // debug!("Reading: {}", std::any::type_name::<W>());
//         self.session.read_object().await
//     }

//     pub(crate) async fn shutdown(self) {
//         self.session.shutdown().await
//     }
// }

// pub struct PendingSession {
//     session: Session,
// }

// impl PendingSession {
//     pub async fn check_authorization<'a>(&'a mut self) -> Result<&'a mut
// Session> {         let perm_result: Result<(), ()> =
// self.session.read_object().await?;

//         match perm_result {
//             Ok(_) => Ok(&mut self.session),
//             Err(_) => Err(anyhow!("Rpc-Error: Permission denied")),
//         }
//     }

//     pub async fn write_object<W: Serialize>(&mut self, object: &W) ->
// Result<()> {         self.session.write_object(object).await
//     }
// }
