use svalin_pki::Certificate;
use svalin_rpc::permissions::{PermissionCheckError, PermissionHandler};

use super::Permission;

/// This still needs a lot of work. It's currently mostly a placeholder.
#[derive(Clone)]
pub struct ServerPermissionHandler {
    root: Certificate,
}

impl ServerPermissionHandler {
    pub fn new(root: Certificate) -> Self {
        Self { root }
    }
}

impl PermissionHandler for ServerPermissionHandler {
    type Permission = Permission;

    async fn may(
        &self,
        peer: &svalin_rpc::rpc::peer::Peer,
        permission: &Permission,
    ) -> anyhow::Result<(), PermissionCheckError> {
        match peer {
            svalin_rpc::rpc::peer::Peer::Certificate(certificate) => {
                if certificate == &self.root {
                    if let Permission::AnonymousOnly = permission {
                        return Err(PermissionCheckError::PermissionDenied(
                            "peer must be unauthenticated for this action! This could be a security critical bug, please report it!".to_string()
                        ));
                    } else {
                        Ok(())
                    }
                } else {
                    match permission {
                        Permission::RootOnlyPlaceholder => Err(PermissionCheckError::PermissionDenied(
                            "only the root certificate is allowed to do that".to_string(),
                        )),
                        Permission::ViewPublicInformation => Ok(()),
                        Permission::AuthenticatedOnly => Ok(()),
                        Permission::AnonymousOnly => Err(PermissionCheckError::PermissionDenied(
                            "peer must be unauthenticated for this action! This could be a security critical bug, please report it!".to_string()
                        )),
                    }
                }
            }
            svalin_rpc::rpc::peer::Peer::Anonymous => match permission {
                Permission::RootOnlyPlaceholder => Err(PermissionCheckError::PermissionDenied(
                    "anonymous peers are not allowed to do that".to_string(),
                )),
                Permission::ViewPublicInformation => Ok(()),
                Permission::AnonymousOnly => Ok(()),
                Permission::AuthenticatedOnly => Err(PermissionCheckError::PermissionDenied(
                    "peer must be authenticated for this action!".to_string(),
                )),
            },
        }
    }
}
