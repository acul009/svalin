use std::{collections::HashMap, fmt::Debug, marker::PhantomData, sync::Arc};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio_util::sync::CancellationToken;

use crate::{
    permissions::PermissionHandler,
    rpc::session::{Session, SessionRequestHeader, SessionResponseHeader},
};

/// This is the default trait meant to be control the server side logic of a
/// command After executing the command, the session is properly closed
#[async_trait]
pub trait CommandHandler: Sync + Send {
    type Request: Send + Serialize + DeserializeOwned;

    fn key() -> String;
    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> Result<()>;
}

/// Some handlers may require taking ownership of the session.
/// This trait is meant to enable that.
/// If the session isn't taken, it will be properly closed
#[async_trait]
pub trait TakeableCommandHandler: Sync + Send {
    type Request: Send + Serialize + DeserializeOwned;

    fn key() -> String;
    async fn handle(
        &self,
        session: &mut Option<Session>,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> Result<()>;
}

#[async_trait]
impl<T> TakeableCommandHandler for T
where
    T: CommandHandler,
{
    type Request = T::Request;

    fn key() -> String {
        Self::key()
    }

    async fn handle(
        &self,
        session: &mut Option<Session>,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> Result<()> {
        if let Some(session) = session {
            self.handle(session, request, cancel).await
        } else {
            Err(anyhow!("tried executing commandhandler with None"))
        }
    }
}

/// This wrapper allows to attach the additional information needed for a
/// permission check to a handler
#[async_trait]
pub trait HandlerPermissionWrapper<P>: Send + Sync
where
    P: PermissionHandler,
{
    async fn handle_with_permission(
        &self,
        session: Session,
        permission_handler: &P,
        cancel: CancellationToken,
    ) -> Result<()>;
}

/// This struct is used as a basis to enable conversion to a permission.
/// The RPC-System itself doesn't provide the permission itself, nor the means
/// to check if a Peer has it.
pub struct PermissionPrecursor<H>
where
    H: TakeableCommandHandler,
{
    request: H::Request,
    handler: PhantomData<H>,
}

/// I'm very sorry if you have to touch this mess of Types.
/// Basically it creates a PermissionPrecursor, then converts it to a Permission
/// and finally checks it using a PermissionHandler
///
/// PermissionHandler as well as the Permission and it's conversion from a
/// precursor have to be provided by the caller
#[async_trait]
impl<H, P> HandlerPermissionWrapper<P> for H
where
    H: TakeableCommandHandler,
    H::Request: DeserializeOwned,
    P: PermissionHandler,
    P::Permission: for<'a> From<&'a PermissionPrecursor<Self>> + Send + Sync,
{
    async fn handle_with_permission(
        &self,
        mut session: Session,
        permission_handler: &P,
        cancel: CancellationToken,
    ) -> Result<()> {
        let request: H::Request = session.read_object().await?;

        let precursor = PermissionPrecursor {
            request,
            handler: PhantomData::<Self>,
        };

        let permission: P::Permission = (&precursor).into();

        let request = precursor.request;

        if let Err(err) = permission_handler.may(session.peer(), &permission).await {
            session
                .write_object(&SessionResponseHeader::Decline {
                    code: 403,
                    message: "Permission denied".into(),
                })
                .await?;
            return Err(err.into());
        }

        session.write_object(&SessionResponseHeader::Accept).await?;

        let mut opt = Some(session);

        let handle_error = self.handle(&mut opt, request, cancel).await;

        if let Some(session) = opt {
            // Todo: handle error somehow?
            session.shutdown().await;
        }

        handle_error
    }
}

/// This struct collects possible handlers and combines them with a
/// PermissionHandler
pub struct HandlerCollection<P>
where
    P: PermissionHandler,
{
    commands: Arc<RwLock<HashMap<String, Arc<dyn HandlerPermissionWrapper<P>>>>>,
    permission_handler: P,
}

impl<P: PermissionHandler> Debug for HandlerCollection<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandlerCollection").finish()
    }
}

impl<P> Clone for HandlerCollection<P>
where
    P: PermissionHandler,
{
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            permission_handler: self.permission_handler.clone(),
        }
    }
}

impl<P> HandlerCollection<P>
where
    P: PermissionHandler,
{
    pub fn new(permission_handler: P) -> Self {
        Self {
            commands: Arc::new(RwLock::new(HashMap::new())),
            permission_handler,
        }
    }

    pub async fn chain(&self) -> ChainCommandAdder<P> {
        let lock = self.commands.write().await;
        ChainCommandAdder { lock }
    }

    pub async fn handle_session(
        &self,
        mut session: Session,
        request_header: SessionRequestHeader,
        cancel: CancellationToken,
    ) -> Result<()> {
        if let Some(handler) = self.commands.read().await.get(&request_header.command_key) {
            handler
                .handle_with_permission(session, &self.permission_handler, cancel)
                .await
        } else {
            session
                .write_object(&SessionResponseHeader::Decline {
                    code: 404,
                    message: "command not found".into(),
                })
                .await?;

            Err(anyhow!("command not found"))
        }
    }
}

/// This is just a helper struct to enable adding multiple handlers using method
/// chaining
pub struct ChainCommandAdder<'a, P> {
    lock: RwLockWriteGuard<'a, HashMap<String, Arc<dyn HandlerPermissionWrapper<P>>>>,
}

impl<'a, P> ChainCommandAdder<'a, P> {
    pub fn add<T>(&mut self, command: T) -> &mut Self
    where
        P: PermissionHandler,
        T: TakeableCommandHandler + 'static,
        P::Permission: for<'b> From<&'b PermissionPrecursor<T>> + Send + Sync,
    {
        self.lock.insert(T::key(), Arc::new(command));
        self
    }
}
