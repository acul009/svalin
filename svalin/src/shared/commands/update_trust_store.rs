use std::sync::{Arc, RwLock};

use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_client_store::ClientStore;
use svalin_pki::{
    secure_chain::{ChainDigest, UncheckedBlock},
    trust_store::{self, TrustStore},
};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use tokio::{select, sync::oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Serialize, Deserialize)]
pub enum TrustStoreUpdate {
    Transaction(Arc<UncheckedBlock<trust_store::Transaction>>),
    UpToDate(ChainDigest),
    Close,
}

pub struct UpdateTrustStore {
    trust_store: Arc<RwLock<TrustStore>>,
    store: Arc<ClientStore>,
    sequence: u64,
    ready: oneshot::Sender<()>,
    cancel: CancellationToken,
}

impl UpdateTrustStore {
    pub fn new(
        trust_store: Arc<RwLock<TrustStore>>,
        store: Arc<ClientStore>,
        ready: oneshot::Sender<()>,
        cancel: CancellationToken,
    ) -> Self {
        let sequence = trust_store.read().unwrap().sequence();
        Self {
            trust_store,
            store,
            sequence,
            ready,
            cancel,
        }
    }
}

impl CommandDispatcher for UpdateTrustStore {
    type Output = ();

    type Error = anyhow::Error;

    type Request = u64;

    fn key() -> String {
        "update-trust-store".into()
    }

    fn get_request(&self) -> &Self::Request {
        &self.sequence
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        // load all newer blocks from server
        loop {
            let update: TrustStoreUpdate = session.read_object().await?;
            match update {
                TrustStoreUpdate::Transaction(unchecked_block) => {
                    apply_block(
                        Arc::into_inner(unchecked_block).expect("arc has not been clones yet"),
                        &self.store.transaction_store(),
                        &self.trust_store,
                    )
                    .await?;
                }
                TrustStoreUpdate::UpToDate(server_digest) => {
                    if server_digest != self.trust_store.read().unwrap().digest() {
                        return Err(anyhow!("server sent wrong digest"));
                    }
                    break;
                }
                TrustStoreUpdate::Close => {
                    return Err(anyhow!("server closed while receiving old blocks"));
                }
            }
        }

        let _ = self.ready.send(());

        let mut update_fut = session.read_object::<TrustStoreUpdate>();

        loop {
            select! {
                _ = self.cancel.cancelled() => {
                    return Ok(())
                }
                update = update_fut => {
                    let update = update?;
                    match update {
                        TrustStoreUpdate::Transaction(unchecked_block) => {
                            apply_block(
                                Arc::into_inner(unchecked_block).expect("arc has not been clones yet"),
                                &self.store.transaction_store(),
                                &self.trust_store,
                            )
                            .await?;
                        },
                        TrustStoreUpdate::UpToDate(_) => return Err(anyhow!("server already sent up to date info")),
                        TrustStoreUpdate::Close => return Ok(()),
                    }
                    update_fut = session.read_object::<TrustStoreUpdate>();
                }
            }
        }
    }
}

async fn apply_block(
    block: UncheckedBlock<trust_store::Transaction>,
    store: &svalin_client_store::trust_store_transaction_store::TrustStoreTransactionStore,
    trust_store: &RwLock<TrustStore>,
) -> anyhow::Result<()> {
    let block = {
        let mut guard = trust_store.write().unwrap();
        guard.check(block)?
    };

    store.add(&block).await?;

    {
        let mut guard = trust_store.write().unwrap();
        guard.apply(block);
    }

    Ok(())
}

pub struct UpdateTrustStoreHandler {
    trust_store: Arc<RwLock<TrustStore>>,
    store: Arc<svalin_server_store::TrustStoreTransactionStore>,
}

impl UpdateTrustStoreHandler {
    pub fn new(
        trust_store: Arc<RwLock<TrustStore>>,
        store: Arc<svalin_server_store::TrustStoreTransactionStore>,
    ) -> Self {
        Self { trust_store, store }
    }
}

#[async_trait]
impl CommandHandler for UpdateTrustStoreHandler {
    type Request = u64;

    fn key() -> String {
        UpdateTrustStore::key()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let (current, mut receiver) = self.store.load_all_after(request).await?;
        for block in current {
            session
                .write_object(&TrustStoreUpdate::Transaction(Arc::new(block)))
                .await?;
        }
        while let Ok(block) = receiver.try_recv() {
            session
                .write_object(&TrustStoreUpdate::Transaction(block))
                .await?;
        }
        let digest = self.trust_store.read().unwrap().digest();
        session
            .write_object(&TrustStoreUpdate::UpToDate(digest))
            .await?;

        loop {
            select! {
                _ = cancel.cancelled() => {
                    session.write_object(&TrustStoreUpdate::Close).await?;
                    return Ok(());
                }
                block = receiver.recv() => {
                    match block {
                        Ok(block) => {
                            session.write_object(&TrustStoreUpdate::Transaction(block)).await?;
                        }
                        Err(err) => {
                            session.write_object(&TrustStoreUpdate::Close).await?;
                            return Err(err.into());
                        }
                    }
                }
            }
        }
    }
}
