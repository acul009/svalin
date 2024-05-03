use std::sync::Arc;

pub struct UpstreamVerifier {
    root_certificate: svalin_pki::Certificate,
    upstream_certificate: svalin_pki::Certificate,
}

impl UpstreamVerifier {
    pub fn new(
        root_certificate: svalin_pki::Certificate,
        upstream_certificate: svalin_pki::Certificate,
    ) -> Arc<Self> {
        Arc::new(Self {
            root_certificate,
            upstream_certificate,
        })
    }
}

impl rustls::client::ServerCertVerifier for UpstreamVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::Certificate,
        intermediates: &[rustls::Certificate],
        server_name: &rustls::ServerName,
        scts: &mut dyn Iterator<Item = &[u8]>,
        ocsp_response: &[u8],
        now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        if self.upstream_certificate.to_der() != end_entity.0 {
            return Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ));
        }

        // TODO: check that certificate chain only contains the root and is valid
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}
