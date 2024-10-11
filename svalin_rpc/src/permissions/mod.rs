use std::{error::Error, fmt::Display, future::Future};

use anyhow::Result;

use crate::rpc::peer::Peer;

pub mod whitelist_permission_handler;

pub trait PermissionHandler {
    type Permission;

    fn may(
        &self,
        peer: &Peer,
        permission: &Self::Permission,
    ) -> impl Future<Output = Result<(), PermissionCheckError>>;
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
