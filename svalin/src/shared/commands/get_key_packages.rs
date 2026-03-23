use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use svalin_pki::{
    SpkiHash,
    mls::key_package::{KeyPackageError, UnverifiedKeyPackage},
};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError},
};
use tokio_util::sync::CancellationToken;

use crate::server::key_package_store::KeyPackageStore;

pub struct GetKeyPackages(pub Vec<SpkiHash>);

#[derive(Debug, thiserror::Error)]
pub enum GetKeyPackagesDispatcherError {
    #[error("Session read error: {0}")]
    SessionRead(#[from] SessionReadError),
    #[error("Server error")]
    ServerError,
    #[error("server did not return all requested key packages")]
    MissingKeyPackages,
    #[error("Key package verify error: {0}")]
    VerifyError(#[from] KeyPackageError),
}

impl CommandDispatcher for GetKeyPackages {
    type Output = Vec<UnverifiedKeyPackage>;

    type Error = GetKeyPackagesDispatcherError;

    type Request = Vec<SpkiHash>;

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
        // TODO: check if all requested members were returned
        let key_packages: Option<Vec<UnverifiedKeyPackage>> = session.read_object().await?;
        if let Some(packages) = &key_packages {
            let received_packages = packages
                .iter()
                .map(|key_package| key_package.spki_hash())
                .collect::<Result<HashSet<SpkiHash>, _>>()?;
            for requested in self.0 {
                if !received_packages.contains(&requested) {
                    return Err(GetKeyPackagesDispatcherError::MissingKeyPackages);
                }
            }
        }
        key_packages.ok_or_else(|| GetKeyPackagesDispatcherError::ServerError)
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
        match self
            .key_package_store
            .get_key_packages(request.iter())
            .await
        {
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
