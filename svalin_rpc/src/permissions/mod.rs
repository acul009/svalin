use std::{error::Error, fmt::Display, future::Future};

use anyhow::Result;

use crate::rpc::{
    command::handler::{PermissionPrecursor, TakeableCommandHandler},
    peer::Peer,
};

pub mod anonymous_permission_handler;
pub mod whitelist;

pub trait PermissionHandler: Send + Sync + Clone + 'static {
    type Permission: Send + Sync + Clone + 'static;

    fn may(
        &self,
        peer: &Peer,
        permission: &Self::Permission,
    ) -> impl Future<Output = Result<(), PermissionCheckError>> + Send + Sync;
}

#[derive(Debug)]
pub enum PermissionCheckError {
    PermissionDenied(String),
    HandlerError(anyhow::Error),
}

impl Display for PermissionCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionCheckError::PermissionDenied(s) => write!(f, "Permission Denied: {}", s),
            PermissionCheckError::HandlerError(e) => {
                write!(f, "Error during permission check: {}", e)
            }
        }
    }
}

impl From<anyhow::Error> for PermissionCheckError {
    fn from(value: anyhow::Error) -> Self {
        PermissionCheckError::HandlerError(value)
    }
}

impl Error for PermissionCheckError {}

#[derive(Default, Clone)]
pub struct DummyPermission;

impl<H> From<&PermissionPrecursor<H>> for DummyPermission
where
    H: TakeableCommandHandler,
{
    fn from(_value: &PermissionPrecursor<H>) -> Self {
        Self
    }
}
