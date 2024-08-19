use std::{
    ops::Deref,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use anyhow::Result;
use svalin_rpc::{
    commands::{forward::ForwardConnection, ping::pingDispatcher},
    rpc::connection::DirectConnection,
};
use svalin_sysctl::realtime::RealtimeStatus;
use tokio::{sync::watch, task::JoinHandle};
use tracing::{debug, error};

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
        {
            let mut current = self.data.item.write().unwrap();

            debug!(
                "updating device status: {}: {}",
                item.public_data.name,
                if item.online_status {
                    "online"
                } else {
                    "offline"
                }
            );

            *current = item;
        }

        if !self.data.realtime.is_closed() {
            self.start_realtime_subscriber_if_neccesary().await;
        }
    }

    pub async fn ping(&self) -> Result<Duration> {
        self.data.connection.ping().await
    }

    pub async fn item(&self) -> AgentListItem {
        self.data.item.read().unwrap().clone()
    }

    async fn start_realtime_subscriber_if_neccesary(&self) {
        if self.data.item.read().unwrap().online_status == false {
            debug!("unable to fetch realtime - device offline");
            return;
        }

        if self.data.realtime.is_closed() {
            debug!("no one listening for realtime updates");
        }

        let mut lock = self.data.realtime_task.lock().unwrap();

        if let Some(handle) = lock.deref() {
            if !handle.is_finished() {
                debug!("realtime monitor already running");
                return;
            }
        }

        let _ = self.data.realtime.send(RemoteLiveData::Pending);

        let conn = self.data.connection.clone();
        let realtime = self.data.realtime.clone();

        debug!("no realtime monitor left, starting new one");

        *lock = Some(tokio::spawn(async move {
            match conn.subscribe_realtime_status(&realtime).await {
                Ok(_) => {
                    debug!("no one left listening to realtime status");
                    let _ = realtime.send(RemoteLiveData::Unavailable);
                }
                Err(err) => {
                    error!("{err}");
                    let _ = realtime.send(RemoteLiveData::Unavailable);
                }
            }
        }));
    }

    pub async fn subscribe_realtime(&self) -> watch::Receiver<RemoteLiveData<RealtimeStatus>> {
        let receiver = self.data.realtime.subscribe();
        self.start_realtime_subscriber_if_neccesary().await;
        receiver
    }
}
