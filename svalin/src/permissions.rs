use svalin_rpc::{
    commands::{
        deauthenticate::DeauthenticateHandler, e2e::E2EHandler, forward::ForwardHandler,
        ping::PingHandler,
    },
    permissions::PermissionHandler,
    rpc::command::handler::PermissionPrecursor,
    rustls::server::danger::ClientCertVerifier,
};

use crate::shared::commands::{
    get_key_packages::GetKeyPackagesHandler, load_certificate_chain::LoadCertificateChainHandler,
    mls::upload_mls::UploadMlsHandler, public_server_status::PublicStatusHandler,
    upload_key_packages::UploadKeyPackagesHandler,
};

pub mod default_permission_handler;

#[derive(Clone)]
pub enum Permission {
    RootOnlyPlaceholder,
    AgentOnlyPlaceholder,
    UserOrSession,
    ViewPublicInformation,
    AuthenticatedOnly,
    AnonymousOnly,
}

impl From<&PermissionPrecursor<PingHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<PingHandler>) -> Self {
        Permission::ViewPublicInformation
    }
}

impl From<&PermissionPrecursor<PublicStatusHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<PublicStatusHandler>) -> Self {
        Permission::ViewPublicInformation
    }
}

impl From<&PermissionPrecursor<ForwardHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<ForwardHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

impl From<&PermissionPrecursor<LoadCertificateChainHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<LoadCertificateChainHandler>) -> Self {
        Self::AuthenticatedOnly
    }
}

impl<Nested, Verifier> From<&PermissionPrecursor<E2EHandler<Nested, Verifier>>> for Permission
where
    Nested: PermissionHandler,
    Verifier: ClientCertVerifier + 'static,
{
    fn from(_value: &PermissionPrecursor<E2EHandler<Nested, Verifier>>) -> Self {
        Permission::AnonymousOnly
    }
}

impl<Nested: PermissionHandler> From<&PermissionPrecursor<DeauthenticateHandler<Nested>>>
    for Permission
{
    fn from(_value: &PermissionPrecursor<DeauthenticateHandler<Nested>>) -> Self {
        Permission::AuthenticatedOnly
    }
}

impl From<&PermissionPrecursor<UploadKeyPackagesHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<UploadKeyPackagesHandler>) -> Self {
        Permission::UserOrSession
    }
}

impl From<&PermissionPrecursor<GetKeyPackagesHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<GetKeyPackagesHandler>) -> Self {
        Permission::UserOrSession
    }
}

impl From<&PermissionPrecursor<UploadMlsHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<UploadMlsHandler>) -> Self {
        Permission::AuthenticatedOnly
    }
}
