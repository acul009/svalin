use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::{RwLock, RwLockWriteGuard};

use crate::{
    permissions::PermissionHandler,
    rpc::session::{Session, SessionRequestHeader, SessionResponseHeader},
};

#[async_trait]
pub trait CommandHandler: Sync + Send {
    type Request: Send + Serialize + DeserializeOwned;

    fn key() -> String;
    async fn handle(&self, session: &mut Session, request: Self::Request) -> Result<()>;
}

#[async_trait]
pub trait TakeableCommandHandler: Sync + Send {
    type Request: Send + Serialize + DeserializeOwned;

    fn key() -> String;
    async fn handle(&self, session: &mut Option<Session>, request: Self::Request) -> Result<()>;
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

    async fn handle(&self, session: &mut Option<Session>, request: Self::Request) -> Result<()> {
        if let Some(session) = session {
            self.handle(session, request).await
        } else {
            Err(anyhow!("tried executing commandhandler with None"))
        }
    }
}

#[async_trait]
pub trait HandlerPermissionWrapper<P, Permission>: Send + Sync
where
    P: PermissionHandler<Permission>,
{
    async fn handle_with_permission(&self, session: Session, permission_handler: &P) -> Result<()>;
}

pub struct PermissionPrecursor<R, H> {
    request: R,
    handler: PhantomData<H>,
}

#[async_trait]
impl<H, P, Permission> HandlerPermissionWrapper<P, Permission> for H
where
    H: TakeableCommandHandler,
    H::Request: DeserializeOwned,
    P: PermissionHandler<Permission>,
    Permission: for<'a> From<&'a PermissionPrecursor<H::Request, Self>> + Send + Sync,
{
    async fn handle_with_permission(
        &self,
        mut session: Session,
        permission_handler: &P,
    ) -> Result<()> {
        let request: H::Request = session.read_object().await?;

        let precursor = PermissionPrecursor {
            request,
            handler: PhantomData::<Self>,
        };

        let permission: Permission = (&precursor).into();

        let request = precursor.request;

        if let Err(err) = permission_handler.may(session.peer(), &permission).await {
            session
                .write_object(&SessionResponseHeader::Decline {
                    code: 403,
                    message: "Permission denied".into(),
                })
                .await?;
            // Todo: inform client about permission error
            return Err(err.into());
        }

        session.write_object(&SessionResponseHeader::Accept).await?;

        let mut opt = Some(session);

        let handle_error = self.handle(&mut opt, request).await;

        if let Some(session) = opt {
            // Todo: handle error somehow?
            session.shutdown().await;
        }

        handle_error
    }
}

pub struct HandlerCollection<P, Permission>
where
    P: PermissionHandler<Permission>,
{
    commands: Arc<RwLock<HashMap<String, Arc<dyn HandlerPermissionWrapper<P, Permission>>>>>,
    permission_handler: P,
}

impl<P, Permission> Clone for HandlerCollection<P, Permission>
where
    P: PermissionHandler<Permission>,
{
    fn clone(&self) -> Self {
        Self {
            commands: self.commands.clone(),
            permission_handler: self.permission_handler.clone(),
        }
    }
}

impl<P, Permission> HandlerCollection<P, Permission>
where
    P: PermissionHandler<Permission>,
{
    pub fn new(permission_handler: P) -> Self {
        Self {
            commands: Arc::new(RwLock::new(HashMap::new())),
            permission_handler,
        }
    }

    pub async fn chain(&self) -> ChainCommandAdder<P, Permission> {
        let lock = self.commands.write().await;
        ChainCommandAdder { lock }
    }

    pub async fn handle_session(
        &self,
        mut session: Session,
        request_header: SessionRequestHeader,
    ) -> Result<()> {
        if let Some(handler) = self.commands.read().await.get(&request_header.command_key) {
            handler
                .handle_with_permission(session, &self.permission_handler)
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

pub struct ChainCommandAdder<'a, P, Permission> {
    lock: RwLockWriteGuard<'a, HashMap<String, Arc<dyn HandlerPermissionWrapper<P, Permission>>>>,
}

impl<'a, P, Permission> ChainCommandAdder<'a, P, Permission> {
    pub fn add<T>(&mut self, command: T) -> &mut Self
    where
        P: PermissionHandler<Permission>,
        T: TakeableCommandHandler + 'static,
        Permission: for<'b> From<&'b PermissionPrecursor<T::Request, T>> + Send + Sync,
    {
        self.lock.insert(T::key(), Arc::new(command));
        self
    }
}
