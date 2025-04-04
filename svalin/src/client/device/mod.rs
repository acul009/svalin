use std::{fmt::Debug, sync::Arc, time::Duration};

use anyhow::{Result, anyhow};
use svalin_rpc::{
    commands::{forward::ForwardConnection, ping::Ping},
    rpc::connection::{Connection, direct_connection::DirectConnection},
};
use svalin_sysctl::realtime::RealtimeStatus;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use crate::{
    agent::update::{
        InstallationInfo, UpdateChannel, request_available_version::AvailableVersionDispatcher,
        request_installation_info::InstallationInfoDispatcher,
    },
    shared::commands::{
        agent_list::AgentListItem,
        realtime_status::SubscribeRealtimeStatus,
        terminal::{RemoteTerminal, RemoteTerminalDispatcher},
    },
    util::smart_subscriber::{SmartSubscriber, SubscriberStarter},
};

use super::tunnel_manager::{TunnelConfig, TunnelCreateError, TunnelManager};

#[derive(Clone, Debug)]
pub enum RemoteLiveData<T> {
    Unavailable,
    Pending,
    Ready(T),
}

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
    realtime: SmartSubscriber<RealtimeStarter>,
    install_info: SmartSubscriber<InstallInfoStarter>,
}

impl Device {
    pub fn new(
        connection: ForwardConnection<DirectConnection>,
        item: AgentListItem,
        tunnel_manager: TunnelManager,
        cancel: CancellationToken,
    ) -> Self {
        return Self {
            data: Arc::new(DeviceData {
                connection: connection.clone(),
                tunnel_manager,
                item: watch::channel(item).0,
                realtime: SmartSubscriber::new(
                    RealtimeStarter {
                        connection: connection.clone(),
                    },
                    cancel.clone(),
                ),
                install_info: SmartSubscriber::new(InstallInfoStarter { connection }, cancel),
            }),
        };
    }

    pub(crate) fn update(&self, new_item: AgentListItem) {
        if new_item.is_online {
            self.data.realtime.restart_if_offline();
            self.data.install_info.restart_if_offline();
        }

        self.data.item.send_replace(new_item);
    }

    pub async fn ping(&self) -> Result<Duration> {
        self.data
            .connection
            .dispatch(Ping)
            .await
            .map_err(|err| anyhow!(err))
    }

    pub fn item(&self) -> watch::Ref<'_, AgentListItem> {
        self.data.item.borrow()
    }

    pub fn subscribe_item(&self) -> watch::Receiver<AgentListItem> {
        self.data.item.subscribe()
    }

    pub fn subscribe_realtime(&self) -> watch::Receiver<RemoteLiveData<RealtimeStatus>> {
        self.data.realtime.subscribe()
    }

    pub fn subscribe_install_info(&self) -> watch::Receiver<RemoteLiveData<InstallationInfo>> {
        self.data.install_info.subscribe()
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
            .map_err(|err| anyhow!(err))
    }

    pub async fn check_update(&self, channel: UpdateChannel) -> Result<String> {
        let update = self
            .data
            .connection
            .dispatch(AvailableVersionDispatcher { channel })
            .await
            .map_err(|err| anyhow!(err));

        if let Ok(version) = &update {
            debug!("version: {}", version);
        };

        update
    }
}

struct InstallInfoStarter {
    connection: ForwardConnection<DirectConnection>,
}

impl SubscriberStarter for InstallInfoStarter {
    type Item = RemoteLiveData<InstallationInfo>;

    fn default(&self) -> Self::Item {
        RemoteLiveData::Unavailable
    }

    fn start(
        &self,
        send: watch::Sender<Self::Item>,
        _cancel: CancellationToken,
    ) -> impl Future<Output = ()> + Send + 'static {
        let connection = self.connection.clone();
        let _ = send.send(RemoteLiveData::Pending);

        async move {
            let send2 = send.clone();
            if let Err(err) = connection
                .dispatch(InstallationInfoDispatcher { send })
                .await
            {
                let _ = send2.send(RemoteLiveData::Unavailable);
                error!("error while requesting InstallationInfo: {err}");
            }
        }
    }
}

struct RealtimeStarter {
    connection: ForwardConnection<DirectConnection>,
}

impl SubscriberStarter for RealtimeStarter {
    type Item = RemoteLiveData<RealtimeStatus>;

    fn default(&self) -> Self::Item {
        RemoteLiveData::Unavailable
    }

    fn start(
        &self,
        send: watch::Sender<Self::Item>,
        cancel: CancellationToken,
    ) -> impl Future<Output = ()> + Send + 'static {
        let connection = self.connection.clone();
        let _ = send.send(RemoteLiveData::Pending);

        async move {
            let send2 = send.clone();
            if let Err(err) = connection
                .dispatch(SubscribeRealtimeStatus { cancel, send })
                .await
            {
                let _ = send2.send(RemoteLiveData::Unavailable);
                error!("error while requesting InstallationInfo: {err}");
            }
        }
    }
}
