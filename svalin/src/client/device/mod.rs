use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Result;
use svalin_rpc::{
    commands::{forward::ForwardConnection, ping::Ping},
    rpc::connection::{direct_connection::DirectConnection, Connection},
};
use svalin_sysctl::realtime::RealtimeStatus;
use tokio::sync::{oneshot, watch};
use tracing::{debug, error};

use crate::shared::{
    commands::{
        agent_list::AgentListItem,
        realtime_status::SubscribeRealtimeStatus,
        terminal::{RemoteTerminal, RemoteTerminalDispatcher},
    },
    lazy_watch::{self, LazyWatch},
};

#[derive(Clone)]
pub enum RemoteLiveData<T> {
    Unavailable,
    Pending,
    Ready(T),
}

impl<T> RemoteLiveData<T> {
    pub fn is_pending(&self) -> bool {
        match self {
            RemoteLiveData::Pending => true,
            _ => false,
        }
    }

    pub fn is_unavailable(&self) -> bool {
        match self {
            Self::Unavailable => true,
            _ => false,
        }
    }

    pub fn get_ready(self) -> Option<T> {
        match self {
            Self::Ready(data) => Some(data),
            _ => None,
        }
    }
}

pub type RealtimeStatusReceiver =
    lazy_watch::Receiver<RemoteLiveData<RealtimeStatus>, RealtimeStatusWatchHandler>;

#[derive(Clone)]
pub struct Device {
    data: Arc<DeviceData>,
}

struct DeviceData {
    connection: ForwardConnection<DirectConnection>,
    item: RwLock<AgentListItem>,
    realtime: LazyWatch<RemoteLiveData<RealtimeStatus>, RealtimeStatusWatchHandler>,
}

impl Device {
    pub fn new(connection: ForwardConnection<DirectConnection>, item: AgentListItem) -> Self {
        let item = RwLock::new(item);

        return Self {
            data: Arc::new(DeviceData {
                connection: connection.clone(),
                item,
                realtime: LazyWatch::new(
                    RemoteLiveData::Unavailable,
                    RealtimeStatusWatchHandler::new(connection),
                ),
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
    }

    pub async fn ping(&self) -> Result<Duration> {
        self.data.connection.dispatch(Ping).await
    }

    pub async fn item(&self) -> AgentListItem {
        self.data.item.read().unwrap().clone()
    }

    pub async fn subscribe_realtime(&self) -> RealtimeStatusReceiver {
        self.data.realtime.subscribe()
    }

    pub async fn open_terminal(&self) -> Result<RemoteTerminal> {
        self.data
            .connection
            .dispatch(RemoteTerminalDispatcher)
            .await
    }
}

pub struct RealtimeStatusWatchHandler {
    connection: ForwardConnection<DirectConnection>,
    stop: Option<oneshot::Sender<()>>,
}

impl RealtimeStatusWatchHandler {
    fn new(connection: ForwardConnection<DirectConnection>) -> Self {
        Self {
            connection,
            stop: None,
        }
    }
}

impl lazy_watch::Handler for RealtimeStatusWatchHandler {
    type T = RemoteLiveData<RealtimeStatus>;

    fn start(&mut self, send: &watch::Sender<Self::T>) {
        let connection = self.connection.clone();
        let _ = send.send(RemoteLiveData::Pending);
        let (stop_send, stop_recv) = oneshot::channel();
        self.stop = Some(stop_send);
        let send = send.clone();

        tokio::spawn(async move {
            let mut stop_recv = stop_recv;
            if stop_recv.try_recv().is_ok() {
                return;
            }

            if let Err(err) = connection
                .dispatch(SubscribeRealtimeStatus {
                    send: send.clone(),
                    stop: stop_recv,
                })
                .await
            {
                let _ = send.send(RemoteLiveData::Unavailable);
                error!("error while receiving RealtimeStatus: {err}");
            };
        });
    }

    fn stop(&mut self) {
        if let Some(channel) = self.stop.take() {
            channel.send(());
        }
    }
}
