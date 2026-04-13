use std::{sync::Arc, time::Duration};

use svalin_pki::{
    ExactVerififier, Verifier,
    mls::{
        agent::MlsAgent,
        client::MlsClient,
        key_package::{KeyPackage, UnverifiedKeyPackage},
        processor::CreateKeyPackageError,
    },
};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError, SessionWriteError},
};
use svalin_server_store::KeyPackageStore;
use tokio_util::sync::CancellationToken;

use crate::{
    remote_key_retriever::RemoteKeyRetriever, server::MlsServer,
    verifier::remote_verifier::RemoteVerifier,
};

pub enum Uploader {
    Client(Arc<MlsClient<RemoteKeyRetriever, RemoteVerifier>>),
    Agent(Arc<MlsAgent<RemoteKeyRetriever, RemoteVerifier>>),
}

impl Uploader {
    async fn create_key_package(&self) -> Result<KeyPackage, CreateKeyPackageError> {
        match self {
            Uploader::Client(client) => client.create_key_package().await,
            Uploader::Agent(agent) => agent.create_key_package().await,
        }
    }
}

impl From<Arc<MlsClient<RemoteKeyRetriever, RemoteVerifier>>> for Uploader {
    fn from(client: Arc<MlsClient<RemoteKeyRetriever, RemoteVerifier>>) -> Self {
        Uploader::Client(client)
    }
}

impl From<Arc<MlsAgent<RemoteKeyRetriever, RemoteVerifier>>> for Uploader {
    fn from(agent: Arc<MlsAgent<RemoteKeyRetriever, RemoteVerifier>>) -> Self {
        Uploader::Agent(agent)
    }
}

pub struct UploadKeyPackages {
    uploader: Uploader,
    cancel: CancellationToken,
}

impl UploadKeyPackages {
    pub fn new(uploader: impl Into<Uploader>, cancel: CancellationToken) -> Self {
        Self {
            uploader: uploader.into(),
            cancel,
        }
    }
}

const TARGET_KEY_PACKAGE_COUNT: u64 = 10;

#[derive(Debug, thiserror::Error)]
pub enum UploadKeyPackagesError {
    #[error("error creating key package: {0}")]
    CreateKeyPackageError(#[from] CreateKeyPackageError),
    #[error("error reading server response: {0}")]
    SessionReadError(#[from] SessionReadError),
    #[error("error sending data to server: {0}")]
    SessionWriteError(#[from] SessionWriteError),
    #[error("Server error")]
    ServerError,
}

impl CommandDispatcher for UploadKeyPackages {
    type Output = ();

    type Request = ();
    type Error = UploadKeyPackagesError;

    fn key() -> String {
        "upload_key_packages".to_string()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(
        self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        loop {
            let current_key_packages = self
                .cancel
                .run_until_cancelled(session.read_object::<u64>())
                .await;

            let Some(current_key_packages) = current_key_packages else {
                return Ok(());
            };
            let mut current_key_packages = current_key_packages?;

            let mut new_packages = Vec::new();
            while current_key_packages < TARGET_KEY_PACKAGE_COUNT {
                let new_package = self.uploader.create_key_package().await?;
                new_packages.push(new_package.to_unverified());
                current_key_packages += 1;
            }

            session.write_object(&new_packages).await?;

            let result: Result<(), ()> = session.read_object().await?;
            result.map_err(|_| UploadKeyPackagesError::ServerError)?;

            if self
                .cancel
                // Todo: figure out something more elegant than polling
                .run_until_cancelled(tokio::time::sleep(Duration::from_mins(1)))
                .await
                .is_none()
            {
                return Ok(());
            }
        }
    }
}

pub struct UploadKeyPackagesHandler {
    pub key_package_store: Arc<KeyPackageStore>,
    pub mls_server: Arc<MlsServer>,
}

#[async_trait::async_trait]
impl CommandHandler for UploadKeyPackagesHandler {
    type Request = ();

    fn key() -> String {
        UploadKeyPackages::key()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let certificate = session.peer().certificate()?.clone();

        let count = self
            .key_package_store
            .count_key_packages(certificate.spki_hash())
            .await?;
        session.write_object(&count).await?;

        let new_packages: Vec<UnverifiedKeyPackage> = session.read_object().await?;

        let verifier = ExactVerififier::new(certificate);
        for unverified in new_packages {
            {
                if let Err(error) = self.verify_and_save(&verifier, unverified).await {
                    tracing::error!("Failed to verify and save key package: {}", error);
                    let _ = session.write_object(&Result::<(), ()>::Err(())).await;
                    return Err(error);
                }
            }
        }
        session.write_object(&Result::<(), ()>::Ok(())).await?;
        Ok(())
    }
}

impl UploadKeyPackagesHandler {
    async fn verify_and_save(
        &self,
        verifier: &impl Verifier,
        unverified: UnverifiedKeyPackage,
    ) -> anyhow::Result<()> {
        let verified = self
            .mls_server
            .verify_key_package(unverified, verifier)
            .await?;
        self.key_package_store.add_key_package(verified).await?;
        Ok(())
    }
}
