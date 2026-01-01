use std::fmt::Debug;

use svalin_pki::{CertificateType, RootCertificate, SpkiHash, Verifier, VerifyError};
use svalin_rpc::rpc::connection::{Connection, direct_connection::DirectConnection};

use crate::verifier::load_certificate_chain::ChainRequest;

pub struct RemoteSessionVerifier {
    root: RootCertificate,
    connection: DirectConnection,
}

impl Debug for RemoteSessionVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSessionVerifier").finish()
    }
}

impl RemoteSessionVerifier {
    pub fn new(root: RootCertificate, connection: DirectConnection) -> Self {
        Self { root, connection }
    }
}

impl Verifier for RemoteSessionVerifier {
    async fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> Result<svalin_pki::Certificate, svalin_pki::VerifyError> {
        tracing::debug!("entering remote session verifier");
        let unverified_chain = self
            .connection
            .dispatch(ChainRequest::Session(spki_hash.clone()))
            .await
            .map_err(|err| VerifyError::InternalError(err.into()))?;

        let chain = unverified_chain.verify(&self.root, time)?;

        let leaf = chain.take_leaf();

        tracing::debug!("exiting remote session verifier");
        if leaf.certificate_type() == CertificateType::UserDevice {
            Ok(leaf)
        } else {
            Err(VerifyError::IncorrectCertificateType)
        }
    }
}
