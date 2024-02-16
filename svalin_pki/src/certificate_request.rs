use anyhow::Result;
use rcgen::CertificateSigningRequest;

pub struct CertificateRequest {
    csr: CertificateSigningRequest,
    raw: String,
}

impl CertificateRequest {
    pub fn from_string(string: String) -> Result<Self> {
        let csr = CertificateSigningRequest::from_pem(&string)?;

        // Todo: verify subject format and check if UUID is in use

        Ok(Self { csr, raw: string })
    }
}
