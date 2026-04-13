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
use svalin_server_store::KeyPackageStore;
use tokio_util::sync::CancellationToken;

pub struct GetKeyPackages(pub Vec<SpkiHash>);

#[derive(Debug, thiserror::Error)]
pub enum GetKeyPackagesDispatcherError {
    #[error("Session read error: {0}")]
    SessionRead(#[from] SessionReadError),
    #[error("Server error")]
    ServerError,
    #[error("server did not return all requested key packages, missing: {0}")]
    MissingKeyPackage(SpkiHash),
    #[error("Key package verify error: {0}")]
    VerifyError(#[from] KeyPackageError),
    #[error("Too many key packages returned")]
    ToManyKeyPackages,
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
            // tracing::debug!("received key packages: {:?}", received_packages);
            if received_packages.len() > packages.len() {
                return Err(GetKeyPackagesDispatcherError::ToManyKeyPackages);
            }
            for requested in self.0 {
                if !received_packages.contains(&requested) {
                    return Err(GetKeyPackagesDispatcherError::MissingKeyPackage(requested));
                }
            }
        }
        key_packages.ok_or_else(|| GetKeyPackagesDispatcherError::ServerError)
    }
}

pub struct GetKeyPackagesHandler {
    pub key_package_store: Arc<KeyPackageStore>,
}

#[async_trait]
impl CommandHandler for GetKeyPackagesHandler {
    type Request = Vec<SpkiHash>;

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
                // tracing::debug!("sending key packages: {:?}", key_packages);
                session.write_object(&Some(key_packages)).await?;
                Ok(())
            }
            Err(err) => {
                let _ = session
                    .write_object(&None::<Vec<UnverifiedKeyPackage>>)
                    .await?;
                Err(err.into())
            }
        }
    }
}
