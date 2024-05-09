use anyhow::Result;
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, PermCredentials};

#[derive(Serialize, Deserialize)]
pub(crate) struct Profile {
    pub(crate) username: String,
    pub(crate) upstream_address: String,
    pub(crate) upstream_certificate: Certificate,
    pub(crate) root_certificate: Certificate,
    pub(crate) raw_credentials: Vec<u8>,
}

impl Profile {
    pub(crate) fn new(
        username: String,
        upstream_address: String,
        upstream_certificate: Certificate,
        root_certificate: Certificate,
        raw_credentials: Vec<u8>,
    ) -> Self {
        Self {
            username,
            upstream_address,
            upstream_certificate,
            root_certificate,
            raw_credentials,
        }
    }
}
