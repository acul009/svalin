use std::{fmt::Debug, sync::Arc};

use dashmap::DashMap;
use svalin_pki::{
    Certificate, RootCertificate, SpkiHash, Verifier, VerifyError, get_current_timestamp,
};
use svalin_rpc::rpc::connection::{Connection, direct_connection::DirectConnection};
use tracing::debug;

use crate::{
    shared::commands::load_certificate_chain::ChainRequest,
    verifier::{
        remote_agent_verifier::RemoteAgentVerifier, remote_session_verifier::RemoteSessionVerifier,
    },
};

struct VerifierData {
    root: RootCertificate,
    connection: DirectConnection,
    cache: DashMap<SpkiHash, Certificate>,
}

#[derive(Clone)]
pub struct RemoteVerifier(Arc<VerifierData>);

impl Debug for RemoteVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteVerifier").finish()
    }
}

impl RemoteVerifier {
    pub fn new(root: RootCertificate, connection: DirectConnection) -> Self {
        Self(Arc::new(VerifierData {
            root,
            connection,
            cache: DashMap::new(),
        }))
    }

    pub fn session_only(self) -> RemoteSessionVerifier {
        RemoteSessionVerifier::new(self)
    }

    pub fn agent_only(self) -> RemoteAgentVerifier {
        RemoteAgentVerifier::new(self)
    }
}

impl Verifier for RemoteVerifier {
    async fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> Result<svalin_pki::Certificate, svalin_pki::VerifyError> {
        // tracing::debug!("entering remote agent verifier");
        let mut found = false;
        if let Some(cached) = self.0.cache.get(spki_hash) {
            found = true;
            // debug!("found in cache");
            if cached.check_validity_at(get_current_timestamp()).is_ok() {
                // debug!("returning from cache");
                return Ok(cached.clone());
            }
            // debug!("cache not valid anymore!");
        }
        if found {
            self.0.cache.remove(spki_hash);
        }

        // debug!("dispatching chain request");
        let unverified_chain = self
            .0
            .connection
            .dispatch(ChainRequest(spki_hash.clone()))
            .await
            .map_err(|err| VerifyError::InternalError(err.into()))?;

        // debug!("verifying received chain");
        let chain = unverified_chain.verify(&self.0.root, time)?;

        let leaf = chain.take_leaf();

        if leaf.spki_hash() != spki_hash {
            return Err(VerifyError::CertificateInvalid);
        }

        self.0.cache.insert(leaf.spki_hash().clone(), leaf.clone());

        // tracing::debug!("exiting remote agent verifier");
        Ok(leaf)
    }
}
