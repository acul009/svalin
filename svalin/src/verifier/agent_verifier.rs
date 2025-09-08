use std::{fmt::Debug, sync::Arc};

use svalin_pki::Verifier;
use svalin_rpc::rpc::client::RpcClient;

pub struct AgentVerifier {
    rpc_client: Arc<RpcClient>,
}

impl Debug for AgentVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentVerifier").finish()
    }
}

impl Verifier for AgentVerifier {
    async fn verify_fingerprint(
        &self,
        fingerprint: &svalin_pki::Fingerprint,
        time: u64,
    ) -> Result<svalin_pki::Certificate, svalin_pki::VerificationError> {
        todo!()
    }
}
