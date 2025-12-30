use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use svalin_pki::{SpkiHash, mls::new_member::UnverifiedNewMember};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError},
};
use tokio_util::sync::CancellationToken;

use crate::server::key_package_store::KeyPackageStore;

pub struct GetKeyPackages(pub HashSet<SpkiHash>);

#[derive(Debug, thiserror::Error)]
pub enum GetKeyPackagesError {
    #[error("Session read error: {0}")]
    SessionRead(#[from] SessionReadError),
    #[error("Server error")]
    ServerError,
}

impl CommandDispatcher for GetKeyPackages {
    type Output = Vec<UnverifiedNewMember>;

    type Error = GetKeyPackagesError;

    type Request = HashSet<SpkiHash>;

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
        let key_packages: Option<Vec<UnverifiedNewMember>> = session.read_object().await?;
        key_packages.ok_or_else(|| GetKeyPackagesError::ServerError)
    }
}

pub struct GetKeyPackagesHandler {
    key_package_store: Arc<KeyPackageStore>,
}

impl GetKeyPackagesHandler {
    pub fn new(key_package_store: Arc<KeyPackageStore>) -> Self {
        Self { key_package_store }
    }
}

#[async_trait]
impl CommandHandler for GetKeyPackagesHandler {
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
                    .write_object(&Option::<Vec<UnverifiedNewMember>>::None)
                    .await?;
                Err(err.into())
            }
        }
    }
}
