use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Context;
use svalin_pki::{
    secure_chain::UncheckedBlock,
    trust_store::{self, TrustStore},
};
use svalin_rpc::rpc::connection::Connection;
use tokio::sync::oneshot;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::shared::commands::update_trust_store::UpdateTrustStore;

pub trait Store {
    fn load_all_after(
        &self,
        after: u64,
    ) -> impl Future<Output = anyhow::Result<Vec<UncheckedBlock<trust_store::Transaction>>>>;
}

impl Store for svalin_store::trust_store_transaction_store::TrustStoreTransactionStore {
    async fn load_all_after(
        &self,
        after: u64,
    ) -> anyhow::Result<Vec<UncheckedBlock<trust_store::Transaction>>> {
        Ok(self.load_all_after(after).await?)
    }
}

impl Store for svalin_store::server_store::TrustStoreTransactionStore {
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
    cancel: CancellationToken,
    task_tracker: &TaskTracker,
) -> anyhow::Result<Arc<RwLock<TrustStore>>> {
    let exported = tokio::fs::read(&file_location)
        .await
        .context("failed to read trust store file")?;
    let exported: trust_store::Exported =
        serde_json::from_slice(&exported).context("failed to deserialize trust store")?;

    let mut trust_store = TrustStore::import(exported).context("failed to import trust store")?;
    let transactions = store
        .load_all_after(trust_store.sequence())
        .await
        .context("failed to load trust store transactions")?;

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
                    if let Err(err)  = save_trust_store(&file_location, &exported).await {
                        eprintln!("error during scheduled trust store save: {err}")
                    }
                }
                _ = cancel.cancelled() => {
                    let exported = trust_store.read().unwrap().export();
                    if let Err(err)  = save_trust_store(&file_location, &exported).await {
                        eprintln!("error during scheduled trust store save: {err}")
                    }
                    break;
                }
            }
        }
    });

    Ok(trust_store)
}

/// This function uses the given connection to download updates for the Trust Store.
/// It will return once all current updates have been downloaded and applied,
/// but it will continue to download and apply updates in a background task.
pub async fn update_trust_store(
    trust_store: Arc<RwLock<TrustStore>>,
    store: Arc<svalin_store::trust_store_transaction_store::TrustStoreTransactionStore>,
    connection: impl Connection + 'static,
    cancel: CancellationToken,
    task_tracker: &TaskTracker,
) -> anyhow::Result<()> {
    let (send, recv) = oneshot::channel();

    task_tracker.spawn(async move {
        if let Err(err) = connection
            .dispatch(UpdateTrustStore::new(trust_store, store, send, cancel))
            .await
        {
            eprintln!("Error updating trust store: {}", err);
        }
    });

    recv.await?;

    Ok(())
}

pub async fn save_trust_store(
    file_location: &Path,
    trust_store: &trust_store::Exported,
) -> anyhow::Result<()> {
    let exported = serde_json::to_vec_pretty(&trust_store)?;

    super::files::override_atomic(file_location, &exported).await?;

    Ok(())
}
