use std::fmt::Debug;

use dashmap::DashMap;
use svalin_pki::{
    Certificate, CertificateType, RootCertificate, SpkiHash, Verifier, VerifyError,
    get_current_timestamp,
};
use svalin_rpc::rpc::connection::{Connection, direct_connection::DirectConnection};

use crate::verifier::load_certificate_chain::ChainRequest;

pub struct RemoteAgentVerifier {
    root: RootCertificate,
    connection: DirectConnection,
    cache: DashMap<SpkiHash, Certificate>,
}

impl Debug for RemoteAgentVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSessionVerifier").finish()
    }
}

impl RemoteAgentVerifier {
    pub fn new(root: RootCertificate, connection: DirectConnection) -> Self {
        Self {
            root,
            connection,
            cache: DashMap::new(),
        }
    }
}

impl Verifier for RemoteAgentVerifier {
    async fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> Result<svalin_pki::Certificate, svalin_pki::VerifyError> {
        tracing::debug!("entering remote agent verifier");
        if let Some(cached) = self.cache.get(spki_hash) {
            cached.check_validity_at(get_current_timestamp())?;
            return Ok(cached.clone());
        }

        let unverified_chain = self
            .connection
            .dispatch(ChainRequest::Agent(spki_hash.clone()))
            .await
            .map_err(|err| VerifyError::InternalError(err.into()))?;

        let chain = unverified_chain.verify(&self.root, time)?;

        let leaf = chain.take_leaf();

        if leaf.certificate_type() == CertificateType::Agent {
            self.cache.insert(leaf.spki_hash().clone(), leaf.clone());
            tracing::debug!("exiting remote agent verifier");
            Ok(leaf)
        } else {
            tracing::debug!("exiting remote agent verifier");
            Err(VerifyError::IncorrectCertificateType)
        }
    }
}
