use std::{borrow::BorrowMut, ops::DerefMut, sync::Arc, time::Duration};

use anyhow::Result;
use svalin_pki::Certificate;
use svalin_rpc::{
    commands::{forward::ForwardConnection, ping::pingDispatcher},
    rpc::connection::{self, DirectConnection},
};
use tokio::sync::RwLock;

use crate::shared::commands::agent_list::AgentListItem;

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
}
