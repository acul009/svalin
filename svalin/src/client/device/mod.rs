use std::{fmt::Debug, sync::Arc, time::Duration};

use anyhow::Result;
use svalin_rpc::{
    commands::{forward::ForwardConnection, ping::Ping},
    rpc::connection::{direct_connection::DirectConnection, Connection},
};
use svalin_sysctl::realtime::RealtimeStatus;
use tokio::sync::{oneshot, watch};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::shared::{
    commands::{
        agent_list::AgentListItem,
        realtime_status::SubscribeRealtimeStatus,
        terminal::{RemoteTerminal, RemoteTerminalDispatcher},
    },
    lazy_watch::{self, LazyWatch},
};

use super::tunnel_manager::{TunnelConfig, TunnelCreateError, TunnelManager};

#[derive(Clone, Debug)]
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

impl Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device").finish()
    }
}

struct DeviceData {
    connection: ForwardConnection<DirectConnection>,
    tunnel_manager: TunnelManager,
    item: watch::Sender<AgentListItem>,
    realtime: LazyWatch<RemoteLiveData<RealtimeStatus>, RealtimeStatusWatchHandler>,
}

impl Device {
    pub fn new(
        connection: ForwardConnection<DirectConnection>,
        item: AgentListItem,
        tunnel_manager: TunnelManager,
    ) -> Self {
        return Self {
            data: Arc::new(DeviceData {
                connection: connection.clone(),
                tunnel_manager,
                item: watch::channel(item).0,
                realtime: LazyWatch::new(
                    RemoteLiveData::Unavailable,
                    RealtimeStatusWatchHandler::new(connection),
                ),
            }),
        };
    }

    pub(crate) fn update(&self, new_item: AgentListItem) {
        self.data.item.send_replace(new_item);
    }

    pub async fn ping(&self) -> Result<Duration> {
        self.data.connection.dispatch(Ping).await
    }

    pub fn item(&self) -> watch::Ref<'_, AgentListItem> {
        self.data.item.borrow()
    }

    pub fn watch_item(&self) -> watch::Receiver<AgentListItem> {
        self.data.item.subscribe()
    }

    pub fn subscribe_realtime(&self) -> RealtimeStatusReceiver {
        self.data.realtime.subscribe()
    }

    pub async fn open_tunnel(&self, config: TunnelConfig) -> Result<(), TunnelCreateError> {
        self.data
            .tunnel_manager
            .open(self.data.connection.clone(), config)
            .await
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
    cancel: Option<CancellationToken>,
}

impl RealtimeStatusWatchHandler {
    fn new(connection: ForwardConnection<DirectConnection>) -> Self {
        Self {
            connection,
            cancel: None,
        }
    }
}

impl lazy_watch::Handler for RealtimeStatusWatchHandler {
    type T = RemoteLiveData<RealtimeStatus>;

    fn start(&mut self, send: &watch::Sender<Self::T>) {
        let connection = self.connection.clone();
        let _ = send.send(RemoteLiveData::Pending);
        let cancel = CancellationToken::new();
        self.cancel = Some(cancel.clone());
        let send = send.clone();

        tokio::spawn(async move {
            if let Err(err) = connection
                .dispatch(SubscribeRealtimeStatus {
                    send: send.clone(),
                    cancel: cancel,
                })
                .await
            {
                let _ = send.send(RemoteLiveData::Unavailable);
                error!("error while receiving RealtimeStatus: {err}");
            };
        });
    }

    fn stop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
    }
}
