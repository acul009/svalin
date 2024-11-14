use svalin_pki::Certificate;
use svalin_rpc::{
    commands::{
        deauthenticate::DeauthenticateHandler, e2e::E2EHandler, forward::ForwardHandler,
        ping::PingHandler,
    },
    permissions::PermissionHandler,
    rpc::command::handler::PermissionPrecursor,
};

use crate::shared::commands::{
    add_user::AddUserHandler, public_server_status::PublicStatusHandler,
};

pub mod agent_permission_handler;
pub mod server_permission_handler;

#[derive(Clone)]
pub enum Permission {
    RootOnlyPlaceholder,
    ViewPublicInformation,
    AuthenticatedOnly,
    AnonymousOnly,
}

impl From<&PermissionPrecursor<(), PingHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<(), PingHandler>) -> Self {
        Permission::ViewPublicInformation
    }
}

impl From<&PermissionPrecursor<(), PublicStatusHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<(), PublicStatusHandler>) -> Self {
        Permission::ViewPublicInformation
    }
}

impl From<&PermissionPrecursor<Certificate, ForwardHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<Certificate, ForwardHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

impl<Nested: PermissionHandler, Verifier>
    From<&PermissionPrecursor<(), E2EHandler<Nested, Verifier>>> for Permission
{
    fn from(_value: &PermissionPrecursor<(), E2EHandler<Nested, Verifier>>) -> Self {
        Permission::AnonymousOnly
    }
}

impl<Nested: PermissionHandler> From<&PermissionPrecursor<(), DeauthenticateHandler<Nested>>>
    for Permission
{
    fn from(_value: &PermissionPrecursor<(), DeauthenticateHandler<Nested>>) -> Self {
        Permission::AuthenticatedOnly
    }
}