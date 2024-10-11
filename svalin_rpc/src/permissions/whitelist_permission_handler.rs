use std::{collections::BTreeMap, marker::PhantomData};

use anyhow::{anyhow, Result};
use svalin_pki::Certificate;

use crate::rpc::peer::Peer;

use super::{PermissionCheckError, PermissionHandler};

pub struct WhitelistPermissionHandler<P> {
    whitelist: BTreeMap<[u8; 32], Certificate>,
    permission: PhantomData<P>,
}

impl<P> WhitelistPermissionHandler<P> {
    pub fn new(whitelist: Vec<Certificate>) -> Self {
        let whitelist = whitelist
            .into_iter()
            .map(|c| (c.get_fingerprint(), c))
            .collect();

        Self {
            whitelist: whitelist,
            permission: PhantomData,
        }
    }
}

impl<P> PermissionHandler for WhitelistPermissionHandler<P> {
    type Permission = P;

    async fn may(
        &self,
        peer: &Peer,
        _permission: &Self::Permission,
    ) -> Result<(), PermissionCheckError> {
        if let Peer::Certificate(cert) = peer {
            if let Some(known_cert) = self.whitelist.get(&cert.get_fingerprint()) {
                if known_cert == cert {
                    return Ok(());
                } else {
                    return Err(PermissionCheckError::HandlerError(anyhow!(
                        "Certificate fingerprint collision"
                    )));
                }
            } else {
                return Err(PermissionCheckError::PermissionDenied(
                    "certificate is not whitelisted".to_string(),
                ));
            }
        }

        Err(PermissionCheckError::PermissionDenied(
            "Anonymous peer is not allowed to do that".to_string(),
        ))
    }
}
