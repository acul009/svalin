use std::sync::{Arc, RwLock};

use svalin_pki::{
    mls::{SvalinGroupId, key_retriever::KeyRetriever},
    trust_store::TrustStore,
};
use svalin_rpc::rpc::connection::{Connection, direct_connection::DirectConnection};

use crate::shared::commands::get_key_packages::GetKeyPackages;

#[derive(Clone)]
pub struct RemoteKeyRetriever {
    connection: DirectConnection,
    trust_store: Arc<RwLock<TrustStore>>,
}
impl RemoteKeyRetriever {
    pub(crate) fn new(connection: DirectConnection, trust_store: Arc<RwLock<TrustStore>>) -> Self {
        Self {
            connection,
            trust_store,
        }
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
                    .trust_store
                    .read()
                    .unwrap()
                    .get_certificate_chain(spki_hash)?;

                let required_members = chain.iter().map(|cert| cert.spki_hash().clone()).collect();

                Ok(required_members)
            }
            SvalinGroupId::DeviceMetaGroup(spki_hash) => {
                let chain = self
                    .trust_store
                    .read()
                    .unwrap()
                    .get_certificate_chain(spki_hash)?;
                let required_members = chain
                    .iter()
                    // Skip the device itself
                    .take(chain.iter().len() - 1)
                    .map(|cert| {
                        tracing::trace!("required meta member: {:?}", cert.certificate_type());
                        cert
                    })
                    .map(|cert| cert.spki_hash().clone())
                    .collect();

                tracing::trace!("required members for group {id:?}: {required_members:?}");

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
