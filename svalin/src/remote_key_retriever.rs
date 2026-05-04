use svalin_pki::{
    RootCertificate, get_current_timestamp,
    mls::{SvalinGroupId, key_retriever::KeyRetriever},
};
use svalin_rpc::rpc::connection::{Connection, direct_connection::DirectConnection};

use crate::shared::commands::{
    get_key_packages::GetKeyPackages, load_certificate_chain::ChainRequest,
};

#[derive(Clone)]
pub struct RemoteKeyRetriever {
    connection: DirectConnection,
    root: RootCertificate,
}
impl RemoteKeyRetriever {
    pub(crate) fn new(connection: DirectConnection, root: RootCertificate) -> Self {
        Self { connection, root }
    }
}

impl KeyRetriever for RemoteKeyRetriever {
    type Error = anyhow::Error;

    async fn get_required_group_members(
        &self,
        id: &SvalinGroupId,
    ) -> Result<Vec<svalin_pki::SpkiHash>, Self::Error> {
        match id {
            SvalinGroupId::DeviceGroup(spki_hash) => {
                let chain = self
                    .connection
                    .dispatch(ChainRequest(spki_hash.clone()))
                    .await?;

                let timestamp = get_current_timestamp();
                let chain = chain.verify(&self.root, timestamp)?;
                let required_members = chain.iter().map(|cert| cert.spki_hash().clone()).collect();

                Ok(required_members)
            }
            SvalinGroupId::DeviceMetaGroup(spki_hash) => {
                let chain = self
                    .connection
                    .dispatch(ChainRequest(spki_hash.clone()))
                    .await?;

                let timestamp = get_current_timestamp();
                let chain = chain.verify(&self.root, timestamp)?;
                let required_members = chain
                    .iter()
                    // Skip the device itself
                    .take(chain.iter().len() - 1)
                    .map(|cert| cert.spki_hash().clone())
                    .collect();

                tracing::debug!("required members for group {id:?}: {required_members:?}");

                Ok(required_members)
            }
        }
    }

    async fn get_key_packages(
        &self,
        entities: &[svalin_pki::SpkiHash],
    ) -> Result<Vec<svalin_pki::mls::key_package::UnverifiedKeyPackage>, Self::Error> {
        let key_packages = self
            .connection
            .dispatch(GetKeyPackages(entities.to_vec()))
            .await?;
        Ok(key_packages)
    }
}
