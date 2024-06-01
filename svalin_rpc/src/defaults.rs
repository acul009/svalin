use std::sync::Arc;

use lazy_static::lazy_static;

lazy_static! {
    static ref CRYPTO_PROVIDER: Arc<quinn::rustls::crypto::CryptoProvider> =
        Arc::new(crate::rustls::crypto::ring::default_provider());
}

pub fn crypto_provider() -> Arc<quinn::rustls::crypto::CryptoProvider> {
    CRYPTO_PROVIDER.clone()
}
