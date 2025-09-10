use svalin_pki::{Certificate, CertificateType};
use svalin_rpc::permissions::{PermissionCheckError, PermissionHandler};

use super::Permission;

/// This still needs a lot of work. It's currently mostly a placeholder.
#[derive(Clone)]
pub struct DefaultPermissionHandler {
    root: Certificate,
}

impl DefaultPermissionHandler {
    pub fn new(root: Certificate) -> Self {
        Self { root }
    }
}

impl PermissionHandler for DefaultPermissionHandler {
    type Permission = Permission;

    async fn may(
        &self,
        peer: &svalin_rpc::rpc::peer::Peer,
        permission: &Permission,
    ) -> anyhow::Result<(), PermissionCheckError> {
        match peer {
            svalin_rpc::rpc::peer::Peer::Certificate(certificate) => {
                let allowed = match certificate.certificate_type() {
                    CertificateType::Root => false,
                    CertificateType::User => false,
                    CertificateType::UserDevice => match permission {
                        Permission::RootOnlyPlaceholder => {
                            certificate.issuer() == self.root.spki_hash()
                        }
                        Permission::AuthenticatedOnly => true,
                        Permission::ViewPublicInformation => true,
                        _ => false,
                    },
                    CertificateType::Agent => match permission {
                        Permission::AuthenticatedOnly => true,
                        Permission::ViewPublicInformation => true,
                        Permission::AgentOnlyPlaceholder => true,
                        _ => false,
                    },
                    CertificateType::Server => match permission {
                        Permission::AuthenticatedOnly => true,
                        _ => false,
                    },
                    CertificateType::Temporary => false,
                };
                if allowed {
                    Ok(())
                } else {
                    match permission {
                        Permission::RootOnlyPlaceholder => Err(PermissionCheckError::PermissionDenied(
                            "only the root user is allowed to do that".to_string(),
                        )),
                        Permission::AgentOnlyPlaceholder => Err(PermissionCheckError::PermissionDenied(
                            "only the agents are allowed to do that".to_string(),
                        )),
                        Permission::ViewPublicInformation => Err(PermissionCheckError::PermissionDenied(
                            "everyone is should be allowed to do this, probably a bug".to_string(),
                        )),
                        Permission::AuthenticatedOnly => Err(PermissionCheckError::PermissionDenied(
                            "peer must be authenticated for this action".to_string()
                        )),
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
                Permission::AgentOnlyPlaceholder => Err(PermissionCheckError::PermissionDenied(
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
