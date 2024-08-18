use std::{
    borrow::BorrowMut,
    future::Pending,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use anyhow::Result;
use svalin_pki::Certificate;
use svalin_rpc::{
    commands::{forward::ForwardConnection, ping::pingDispatcher},
    rpc::connection::{self, DirectConnection},
    rustls::lock,
};
use svalin_sysctl::realtime::RealtimeStatus;
use tokio::{
    runtime::Handle,
    sync::{broadcast, watch, RwLock},
    task::JoinHandle,
};
use tracing::{error, instrument::WithSubscriber};

use crate::shared::commands::{
    agent_list::AgentListItem, realtime_status::subscribe_realtime_statusDispatcher,
};

#[derive(Clone)]
pub enum RemoteLiveData<T> {
    Unavailable,
    Pending,
    Ready(T),
}

#[derive(Clone)]
pub struct Device {
    data: Arc<DeviceData>,
}

struct DeviceData {
    connection: ForwardConnection<DirectConnection>,
    item: RwLock<AgentListItem>,
    realtime: watch::Sender<RemoteLiveData<RealtimeStatus>>,
    realtime_task: Mutex<Option<JoinHandle<()>>>,
}

impl Device {
    pub fn new(connection: ForwardConnection<DirectConnection>, item: AgentListItem) -> Self {
        let item = RwLock::new(item);

        let (realtime_send, _realtime_recv) = watch::channel(RemoteLiveData::Unavailable);

        return Self {
            data: Arc::new(DeviceData {
                connection,
                item,
                realtime: realtime_send,
                realtime_task: Mutex::new(None),
            }),
        };
    }

    pub(crate) async fn update(&self, item: AgentListItem) {
        let mut current = self.data.item.write().await;

        if !self.data.realtime.is_closed() {
            self.start_realtime_subscriber_if_neccesary().await;
        }

        *current = item
    }

    pub async fn ping(&self) -> Result<Duration> {
        self.data.connection.ping().await
    }

    pub async fn item(&self) -> AgentListItem {
        self.data.item.read().await.clone()
    }

    async fn start_realtime_subscriber_if_neccesary(&self) {
        if self.data.item.read().await.online_status == false {
            return;
        }

        let mut lock = self.data.realtime_task.lock().unwrap();

        if let Some(handle) = lock.deref() {
            if handle.is_finished() {
                return;
            }
        }

        let _ = self.data.realtime.send(RemoteLiveData::Pending);

        let conn = self.data.connection.clone();
        let realtime = self.data.realtime.clone();

        *lock = Some(tokio::spawn(async move {
            match conn.subscribe_realtime_status(&realtime).await {
                Ok(_) => {}
                Err(_) => {
                    let _ = realtime.send(RemoteLiveData::Unavailable);
                }
            }
        }));
    }

    pub async fn subscribe_realtime(&self) -> watch::Receiver<RemoteLiveData<RealtimeStatus>> {
        self.start_realtime_subscriber_if_neccesary().await;
        self.data.realtime.subscribe()
    }
}
