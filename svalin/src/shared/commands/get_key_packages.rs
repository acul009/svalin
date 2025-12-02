use std::collections::HashMap;

use async_trait::async_trait;
use svalin_pki::{Certificate, mls::NewMember};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use tokio_util::sync::CancellationToken;

use crate::server::user_store::UserStore;

pub struct GetKeyPackages(pub Vec<Certificate>);

#[derive(Debug, thiserror::Error)]
pub enum GetKeyPackagesError {}

impl CommandDispatcher for GetKeyPackages {
    type Output = Vec<NewMember>;

    type Error = GetKeyPackagesError;

    type Request = Vec<Certificate>;

    fn key() -> String {
        GetKeyPackagesHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &self.0
    }

    async fn dispatch(
        self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        todo!()
    }
}

pub struct GetKeyPackagesHandler {}

#[async_trait]
impl CommandHandler for GetKeyPackagesHandler {
    type Request = Vec<Certificate>;

    fn key() -> String {
        "get_key_packages".into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        todo!()
    }
}
