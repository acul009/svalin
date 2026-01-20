use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use svalin_pki::{SpkiHash, mls::key_package::UnverifiedKeyPackage};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError},
};
use tokio_util::sync::CancellationToken;

use crate::server::key_package_store::KeyPackageStore;

pub struct GetKeyPackagesForUsers(pub HashSet<SpkiHash>);

#[derive(Debug, thiserror::Error)]
pub enum GetKeyPackagesError {
    #[error("Session read error: {0}")]
    SessionRead(#[from] SessionReadError),
    #[error("Server error")]
    ServerError,
}

impl CommandDispatcher for GetKeyPackagesForUsers {
    type Output = Vec<UnverifiedKeyPackage>;

    type Error = GetKeyPackagesError;

    type Request = HashSet<SpkiHash>;

    fn key() -> String {
        GetKeyPackagesForUsersHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &self.0
    }

    async fn dispatch(
        self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        let key_packages: Option<Vec<UnverifiedKeyPackage>> = session.read_object().await?;
        key_packages.ok_or_else(|| GetKeyPackagesError::ServerError)
    }
}

pub struct GetKeyPackagesForUsersHandler {
    key_package_store: Arc<KeyPackageStore>,
}

impl GetKeyPackagesForUsersHandler {
    pub fn new(key_package_store: Arc<KeyPackageStore>) -> Self {
        Self { key_package_store }
    }
}

#[async_trait]
impl CommandHandler for GetKeyPackagesForUsersHandler {
    type Request = HashSet<SpkiHash>;

    fn key() -> String {
        "get_key_packages".into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        match self.key_package_store.get_key_packages(&request).await {
            Ok(key_packages) => {
                session.write_object(&Some(key_packages)).await?;
                Ok(())
            }
            Err(err) => {
                let _ = session
                    .write_object(&Option::<Vec<UnverifiedKeyPackage>>::None)
                    .await?;
                Err(err.into())
            }
        }
    }
}
