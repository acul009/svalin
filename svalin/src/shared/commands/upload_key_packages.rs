use std::sync::Arc;

use svalin_pki::{ExactVerififier, mls::new_member::{NewMember, UnverifiedNewMember}};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use tokio_util::sync::CancellationToken;

use crate::server::key_package_store::KeyPackageStore;

pub struct UploadKeyPackages(Vec<UnverifiedNewMember>);

impl UploadKeyPackages {
    pub fn new(members: impl IntoIterator<Item = NewMember>) -> Self {
        Self(
            members
                .into_iter()
                .map(|member| member.to_unverified())
                .collect(),
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UploadKeyPackagesError {}

impl CommandDispatcher for UploadKeyPackages {
    type Output = ();

    type Request = Vec<UnverifiedNewMember>;
    type Error = UploadKeyPackagesError;

    fn key() -> String {
        todo!()
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

pub struct UploadKeyPackagesHandler {
    key_package_store: Arc<KeyPackageStore>,
}

#[async_trait::async_trait]
impl CommandHandler for UploadKeyPackagesHandler {
    type Request = Vec<UnverifiedNewMember>;

    fn key() -> String {
        "upload_key_packages".to_string()
    }
    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let certificate = session.peer().certificate()?;
        let verifier = ExactVerififier::new(certificate.clone());
        let mut members = Vec::new();
        for unverified in request {
            let verified = unverified.verify(crypto, protocol_version, verifier)
        }
        todo!()
    }
}
