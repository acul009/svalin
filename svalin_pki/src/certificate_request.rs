use anyhow::Result;
use rcgen::CertificateSigningRequest;

pub struct CertificateRequest {
    pub(crate) csr: CertificateSigningRequest,
}

impl CertificateRequest {
    pub fn from_string(string: String) -> Result<Self> {
        let csr = CertificateSigningRequest::from_pem(&string)?;

        // Todo: verify subject format and check if UUID is in use

        Ok(Self { csr })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_cert_request_creation() {
        let root = crate::Keypair::generate()
            .unwrap()
            .to_self_signed_cert()
            .unwrap();
        let keypair = crate::Keypair::generate().unwrap();
        let raw_request = keypair.generate_request().unwrap();
        let request = CertificateRequest::from_string(raw_request).unwrap();
        let cert = root.approve_request(request).unwrap();
        let _creds = keypair.upgrade(cert).unwrap();
    }
}
