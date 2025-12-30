use std::{fmt::Debug, sync::Arc};

use svalin_pki::{Certificate, SpkiHash, VerificationError, Verifier};
use svalin_rpc::rpc::{client::RpcClient, connection::Connection};

use crate::verifier::load_session_chain::LoadSessionChain;

pub struct RemoteSessionVerifier {
    root: Certificate,
    rpc_client: Arc<RpcClient>,
}

impl Debug for RemoteSessionVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentVerifier").finish()
    }
}

impl RemoteSessionVerifier {
    pub fn new(root: Certificate, rpc_client: Arc<RpcClient>) -> Self {
        Self { root, rpc_client }
    }
}

impl Verifier for RemoteSessionVerifier {
    async fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> Result<svalin_pki::Certificate, svalin_pki::VerificationError> {
        let chain = self
            .rpc_client
            .upstream_connection()
            .dispatch(LoadSessionChain { spki_hash })
            .await
            .map_err(|err| VerificationError::InternalError(err.into()))?;

        let certificate = chain.verify(&self.root, time)?;

        Ok(certificate.clone())
    }
}
