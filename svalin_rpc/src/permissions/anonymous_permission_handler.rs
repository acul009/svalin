use crate::rpc::peer::Peer;

use super::PermissionHandler;

#[derive(Default, Clone)]
pub struct AnonymousPermissionHandler;

impl<Permission> PermissionHandler<Permission> for AnonymousPermissionHandler
where
    Permission: Send + Sync,
{
    async fn may(
        &self,
        peer: &crate::rpc::peer::Peer,
        _permission: &Permission,
    ) -> anyhow::Result<(), super::PermissionCheckError> {
        match peer {
            Peer::Anonymous => Ok(()),
            Peer::Certificate(_certificate) => Err(super::PermissionCheckError::PermissionDenied(
                "Only anonymous peers are allowed to do that".to_string(),
            )),
        }
    }
}
