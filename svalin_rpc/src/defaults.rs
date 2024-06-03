use std::sync::Arc;

use crate::rustls;

use lazy_static::lazy_static;

lazy_static! {
    static ref CRYPTO_PROVIDER: Arc<rustls::crypto::CryptoProvider> =
        Arc::new(crate::rustls::crypto::ring::default_provider());
}

pub fn crypto_provider() -> Arc<rustls::crypto::CryptoProvider> {
    CRYPTO_PROVIDER.clone()
}
