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
