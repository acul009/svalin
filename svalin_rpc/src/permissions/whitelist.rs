use std::{collections::BTreeMap, marker::PhantomData, sync::Arc};

use anyhow::{Result, anyhow};
use svalin_pki::{Certificate, SpkiHash};

use crate::rpc::peer::Peer;

use super::{PermissionCheckError, PermissionHandler};

pub struct WhitelistPermissionHandler<Permission> {
    whitelist: Arc<BTreeMap<SpkiHash, Certificate>>,

    permission: PhantomData<Permission>,
}

impl<Permission> Clone for WhitelistPermissionHandler<Permission> {
    fn clone(&self) -> Self {
        Self {
            whitelist: self.whitelist.clone(),
            permission: PhantomData,
        }
    }
}

impl<Permission> WhitelistPermissionHandler<Permission> {
    pub fn new(whitelist: Vec<Certificate>) -> Self {
        let whitelist = whitelist
            .into_iter()
            .map(|c| (c.spki_hash().clone(), c))
            .collect();

        Self {
            whitelist: Arc::new(whitelist),
            permission: PhantomData,
        }
    }
}

impl<Permission> PermissionHandler for WhitelistPermissionHandler<Permission>
where
    Permission: Send + Sync + Clone + 'static,
{
    type Permission = Permission;
    async fn may(&self, peer: &Peer, _permission: &Permission) -> Result<(), PermissionCheckError> {
        if let Peer::Certificate(cert) = peer {
            if let Some(known_cert) = self.whitelist.get(&cert.spki_hash()) {
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
