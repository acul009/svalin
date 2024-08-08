use std::sync::{Arc, LazyLock};

use crate::rustls;

static CRYPTO_PROVIDER: LazyLock<Arc<rustls::crypto::CryptoProvider>> =
    LazyLock::new(|| Arc::new(crate::rustls::crypto::ring::default_provider()));

pub fn crypto_provider() -> Arc<rustls::crypto::CryptoProvider> {
    CRYPTO_PROVIDER.clone()
}
