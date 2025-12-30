use svalin_pki::{ExactVerififier, SpkiHash, VerificationError, Verifier};

/// flutter_rust_bridge:ignore
#[derive(Debug)]
pub struct UpstreamVerifier {
    root_certificate: svalin_pki::Certificate,
    verifier: ExactVerififier,
}

impl UpstreamVerifier {
    pub fn new(
        root_certificate: svalin_pki::Certificate,
        upstream_certificate: svalin_pki::Certificate,
    ) -> Self {
        // TODO: verify upstream certificate chain with root
        let verifier = ExactVerififier::new(upstream_certificate);

        Self {
            verifier,
            root_certificate,
        }
    }
}

impl Verifier for UpstreamVerifier {
    async fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> std::result::Result<svalin_pki::Certificate, VerificationError> {
        let cert = self.verifier.verify_spki_hash(spki_hash, time).await?;

        cert.verify_signature(&self.root_certificate)?;

        Ok(cert)
    }
}
