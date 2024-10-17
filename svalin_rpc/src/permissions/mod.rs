use std::{error::Error, fmt::Display, future::Future};

use anyhow::Result;

use crate::rpc::{command::handler::PermissionPrecursor, peer::Peer};

pub mod anonymous_permission_handler;
pub mod whitelist;

pub trait PermissionHandler<Permission>: Send + Sync + Clone + 'static {
    fn may(
        &self,
        peer: &Peer,
        permission: &Permission,
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

pub struct DummyPermission;

impl<R, H> From<&PermissionPrecursor<R, H>> for DummyPermission {
    fn from(_value: &PermissionPrecursor<R, H>) -> Self {
        Self
    }
}
