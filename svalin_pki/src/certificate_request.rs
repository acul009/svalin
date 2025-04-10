use anyhow::Result;
use rcgen::CertificateSigningRequestParams;

pub struct CertificateRequest {
    pub(crate) csr: CertificateSigningRequestParams,
}

#[derive(Debug, thiserror::Error)]
pub enum CertificateRequestParseError {
    #[error("error parsing certificate request: {0}")]
    ParseError(rcgen::Error),
}

impl CertificateRequest {
    pub fn from_string(string: String) -> Result<Self, CertificateRequestParseError> {
        let csr = CertificateSigningRequestParams::from_pem(&string)
            .map_err(CertificateRequestParseError::ParseError)?;

        // Todo: verify subject format and check if key based id is in use

        Ok(Self { csr })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_cert_request_creation() {
        let root = crate::Keypair::generate().to_self_signed_cert().unwrap();
        let keypair = crate::Keypair::generate();
        let raw_request = keypair.generate_request().unwrap();
        let request = CertificateRequest::from_string(raw_request).unwrap();
        let cert = root.approve_request(request).unwrap();
        let _creds = keypair.upgrade(cert).unwrap();
    }
}
