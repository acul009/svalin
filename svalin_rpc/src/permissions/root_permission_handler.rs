use std::marker::PhantomData;

use anyhow::Result;
use svalin_pki::Certificate;

use crate::rpc::peer::Peer;

use super::{PermissionCheckError, PermissionHandler};

pub struct RootPermissionHandler<P> {
    root: Certificate,
    permission: PhantomData<P>,
}

impl<P> RootPermissionHandler<P> {
    pub fn new(root: Certificate) -> Self {
        Self {
            root,
            permission: PhantomData,
        }
    }
}

impl<P> PermissionHandler for RootPermissionHandler<P> {
    type Permission = P;

    async fn may(
        &self,
        peer: &Peer,
        _permission: &Self::Permission,
    ) -> Result<(), PermissionCheckError> {
        if let Peer::Certificate(cert) = peer {
            if cert == &self.root {
                return Ok(());
            } else {
                return Err(PermissionCheckError::PermissionDenied(
                    "Only root is allowed to do that".to_string(),
                ));
            }
        }

        Err(PermissionCheckError::PermissionDenied(
            "Anonymous peer is not allowed to do that".to_string(),
        ))
    }
}
