use std::{
    borrow::BorrowMut,
    ops::DerefMut,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use svalin_pki::Certificate;
use svalin_rpc::{
    commands::{forward::ForwardConnection, ping::pingDispatcher},
    rpc::connection::{self, DirectConnection},
};
use svalin_sysctl::realtime::RealtimeStatus;
use tokio::{
    sync::{broadcast, watch, RwLock},
    task::JoinHandle,
};
use tracing::{error, instrument::WithSubscriber};

use crate::shared::commands::{
    agent_list::AgentListItem, realtime_status::subscribe_realtime_statusDispatcher,
};

#[derive(Clone)]
pub struct Device {
    data: Arc<DeviceData>,
}

struct DeviceData {
    connection: ForwardConnection<DirectConnection>,
    item: RwLock<AgentListItem>,
}

impl Device {
    pub fn new(connection: ForwardConnection<DirectConnection>, item: AgentListItem) -> Self {
        let item = RwLock::new(item);
        return Self {
            data: Arc::new(DeviceData { connection, item }),
        };
    }

    pub(crate) async fn update(&self, item: AgentListItem) {
        let mut current = self.data.item.write().await;
        *current = item
    }

    pub async fn ping(&self) -> Result<Duration> {
        self.data.connection.ping().await
    }

    pub async fn item(&self) -> AgentListItem {
        self.data.item.read().await.clone()
    }

    pub async fn subscribe_realtime(&self) -> watch::Receiver<Option<RealtimeStatus>> {
        let (send, recv) = watch::channel(None);
        let connection = self.data.connection.clone();
        let handle = tokio::spawn(async move {
            if let Err(err) = connection.subscribe_realtime_status(send).await {
                error!("{err}");
            }
        });

        let arc = Arc::new(());

        recv.clone()
    }
}
