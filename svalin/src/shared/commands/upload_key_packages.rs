use std::sync::Arc;

use svalin_pki::{
    ExactVerififier, Verifier,
    mls::{
        self,
        client::{CreateKeyPackageError, MlsClient},
        key_package::UnverifiedKeyPackage,
    },
};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError, SessionWriteError},
};
use tokio_util::sync::CancellationToken;

use crate::server::key_package_store::KeyPackageStore;

pub struct UploadKeyPackages<'a>(pub &'a MlsClient);

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

impl<'a> CommandDispatcher for UploadKeyPackages<'a> {
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
        let mut current_key_packages: u64 = session.read_object().await?;

        let mut new_packages = Vec::new();
        while current_key_packages < TARGET_KEY_PACKAGE_COUNT {
            let new_package = self.0.create_key_package().await?;
            new_packages.push(new_package.to_unverified());
            current_key_packages += 1;
        }

        session.write_object(&new_packages).await?;

        let result: Result<(), ()> = session.read_object().await?;
        result.map_err(|_| UploadKeyPackagesError::ServerError)
    }
}

pub struct UploadKeyPackagesHandler<Crypto> {
    key_package_store: Arc<KeyPackageStore>,
    protocol_version: mls::ProtocolVersion,
    crypto: Crypto,
}

#[async_trait::async_trait]
impl<Crypto> CommandHandler for UploadKeyPackagesHandler<Crypto>
where
    Crypto: svalin_pki::mls::OpenMlsCrypto,
{
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

impl<Crypto> UploadKeyPackagesHandler<Crypto>
where
    Crypto: svalin_pki::mls::OpenMlsCrypto,
{
    async fn verify_and_save(
        &self,
        verifier: &impl Verifier,
        unverified: UnverifiedKeyPackage,
    ) -> anyhow::Result<()> {
        let verified = unverified
            .verify(&self.crypto, self.protocol_version, verifier)
            .await?;
        self.key_package_store.add_key_package(verified).await?;
        Ok(())
    }
}
