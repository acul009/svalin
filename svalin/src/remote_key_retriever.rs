use anyhow::anyhow;
use svalin_pki::{RootCertificate, get_current_timestamp, mls::key_retriever::KeyRetriever};
use svalin_rpc::rpc::{
    client::RpcClient,
    connection::{Connection, direct_connection::DirectConnection},
};

use crate::shared::commands::{
    get_key_packages::GetKeyPackages, list_user_sessions::ListUserSessions,
    load_certificate_chain::ChainRequest,
};

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

    async fn get_required_device_group_members(
        &self,
        device: &svalin_pki::SpkiHash,
    ) -> Result<Vec<svalin_pki::SpkiHash>, Self::Error> {
        let chain = self
            .connection
            .dispatch(ChainRequest(device.clone()))
            .await?;

        let timestamp = get_current_timestamp();

        let chain = chain.verify(&self.root, timestamp)?;

        let mut required_members = vec![device.clone()];

        for user_certificate in chain.iter().rev().skip(1) {
            required_members.push(user_certificate.spki_hash().clone());
            let user_sessions = self
                .connection
                .dispatch(ListUserSessions(user_certificate.spki_hash()))
                .await
                .map_err(|err| anyhow!(err))?;

            for session in user_sessions.into_iter() {
                match session.verify_signature(user_certificate, timestamp) {
                    Ok(certificate) => {
                        required_members.push(certificate.spki_hash().clone());
                    }
                    // TODO: report this error somewhere
                    Err(_) => {}
                }
            }
        }

        Ok(required_members)
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
