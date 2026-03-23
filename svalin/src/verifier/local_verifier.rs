use std::sync::Arc;

use anyhow::anyhow;
use svalin_pki::{
    Certificate, CertificateChainBuilder, RootCertificate, SpkiHash, Verifier, VerifyError,
};

use crate::server::{chain_loader::ChainLoader, user_store::UserStore};

#[derive(Debug, Clone)]
pub struct LocalVerifier {
    root: RootCertificate,
    loader: ChainLoader,
}

impl LocalVerifier {
    pub fn new(root: RootCertificate, loader: ChainLoader) -> Self {
        Self {
            // TODO: user verifier
            root,
            loader,
        }
    }
}

impl Verifier for LocalVerifier {
    async fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> Result<Certificate, VerifyError> {
        let Some(cert_chain) = self.loader.load_certificate_chain(spki_hash).await? else {
            return Err(VerifyError::UnknownCertificate);
        };

        let cert_chain = cert_chain.verify(&self.root, time)?;

        let cert = cert_chain.take_leaf();
        tracing::debug!("exiting incoming connection verifier");
        Ok(cert)
    }
}
