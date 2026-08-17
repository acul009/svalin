use std::sync::{Arc, RwLock};

use svalin_pki::trust_store::TrustStore;

pub struct TrustStoreManager {
    trust_store: Arc<RwLock<TrustStore>>,
}
