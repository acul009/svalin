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
                tracing::debug!(
                    "found required members for device {}: {:?}",
                    spki_hash,
                    required_members
                );

                // This was kind of a bad idea - the client should add it's sessions when they are needed.
                // Otherwise everyone would have access to how many sessions a user has. - sounds like a bad idea.
                //
                // for user_certificate in chain.iter().rev().skip(1) {
                //     required_members.push(user_certificate.spki_hash().clone());
                //     let user_sessions = self
                //         .connection
                //         .dispatch(ListUserSessions(user_certificate.spki_hash()))
                //         .await
                //         .map_err(|err| anyhow!(err))?;

                //     for session in user_sessions.into_iter() {
                //         match session.verify_signature(user_certificate, timestamp) {
                //             Ok(certificate) => {
                //                 required_members.push(certificate.spki_hash().clone());
                //             }
                //             // TODO: report this error somewhere
                //             Err(_) => {}
                //         }
                //     }
                // }

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
