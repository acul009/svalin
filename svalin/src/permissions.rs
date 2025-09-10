use svalin_rpc::{
    commands::{
        deauthenticate::DeauthenticateHandler, e2e::E2EHandler, forward::ForwardHandler,
        ping::PingHandler,
    },
    permissions::PermissionHandler,
    rpc::command::handler::PermissionPrecursor,
    rustls::server::danger::ClientCertVerifier,
};

use crate::{
    shared::commands::public_server_status::PublicStatusHandler,
    verifier::load_session_chain::LoadSessionChainHandler,
};

pub mod default_permission_handler;

#[derive(Clone)]
pub enum Permission {
    RootOnlyPlaceholder,
    AgentOnlyPlaceholder,
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

impl From<&PermissionPrecursor<LoadSessionChainHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<LoadSessionChainHandler>) -> Self {
        Permission::AgentOnlyPlaceholder
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
