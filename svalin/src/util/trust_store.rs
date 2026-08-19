use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};

use svalin_pki::{
    secure_chain::UncheckedBlock,
    trust_store::{self, TrustStore},
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

trait Store {
    fn load_all_after(
        &self,
        after: u64,
    ) -> impl Future<Output = anyhow::Result<Vec<UncheckedBlock<trust_store::Transaction>>>>;
}

impl Store for svalin_client_store::trust_store_transaction_store::TrustStoreTransactionStore {
    async fn load_all_after(
        &self,
        after: u64,
    ) -> anyhow::Result<Vec<UncheckedBlock<trust_store::Transaction>>> {
        Ok(self.load_all_after(after).await?)
    }
}

impl Store for svalin_server_store::TrustStoreTransactionStore {
    async fn load_all_after(
        &self,
        after: u64,
    ) -> anyhow::Result<Vec<UncheckedBlock<trust_store::Transaction>>> {
        Ok(self.load_all_after(after).await?.0)
    }
}

pub async fn load_trust_store(
    file_location: PathBuf,
    store: &impl Store,
    // store: &svalin_client_store::trust_store_transaction_store::TrustStoreTransactionStore,
    cancel: CancellationToken,
    task_tracker: &TaskTracker,
) -> anyhow::Result<Arc<RwLock<TrustStore>>> {
    let exported = tokio::fs::read(&file_location).await?;
    let exported: trust_store::Exported = serde_json::from_slice(&exported)?;

    let mut trust_store = TrustStore::import(exported)?;
    let transactions = store.load_all_after(trust_store.sequence()).await?;

    for block in transactions {
        let block = trust_store.check(block)?;
        trust_store.apply(block);
    }

    let exported = trust_store.export();
    let exported = serde_json::to_vec_pretty(&exported)?;
    tokio::fs::write(&file_location, exported).await?;

    let trust_store = Arc::new(RwLock::new(trust_store));
    let trust_store_2 = trust_store.clone();

    task_tracker.spawn(async move {
        let trust_store = trust_store_2;
        let mut last_digest = trust_store.read().unwrap().digest();
        let file_location = file_location;
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(300)) => {
                    let exported = {
                        let guard = trust_store.read().unwrap();
                        if guard.digest() == last_digest {
                            continue;
                        }
                        last_digest = guard.digest();
                        guard.export()
                    };
                    let Ok(exported) = serde_json::to_vec_pretty(&exported) else {
                        eprintln!("Failed to serialize trust store");
                        continue;
                    };
                    if let Err(e) = tokio::fs::write(&file_location, exported).await {
                        eprintln!("Failed to write trust store: {}", e);
                    }
                }
                _ = cancel.cancelled() => {
                    break;
                }
            }
        }
    });

    Ok(trust_store)
}
