use std::sync::Arc;

use svalin_pki::{Fingerprint, VerificationError, Verifier};

use crate::server::user_store::UserStore;

#[derive(Debug)]
pub struct UserStoreVerifier {
    store: Arc<UserStore>,
}

impl UserStoreVerifier {
    pub fn new(store: Arc<UserStore>) -> Self {
        Self { store }
    }
}

impl Verifier for UserStoreVerifier {
    async fn verify_fingerprint(
        &self,
        fingerprint: &Fingerprint,
        time: u64,
    ) -> Result<svalin_pki::Certificate, VerificationError> {
        todo!()
    }
}
