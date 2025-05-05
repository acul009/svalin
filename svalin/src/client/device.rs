use std::{fmt::Debug, sync::Arc, time::Duration};

use anyhow::{Result, anyhow};
use svalin_rpc::{
    commands::{forward::ForwardConnection, ping::Ping},
    rpc::connection::{Connection, direct_connection::DirectConnection},
};
use svalin_sysctl::{
    pty::{TerminalInput, TerminalSize},
    realtime::RealtimeStatus,
};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    agent::update::{
        InstallationInfo, UpdateChannel, request_available_version::AvailableVersionDispatcher,
        request_installation_info::InstallationInfoDispatcher,
        start_agent_update::StartUpdateDispatcher,
    },
    shared::commands::{
        agent_list::AgentListItem, realtime_status::SubscribeRealtimeStatus,
        terminal::RemoteTerminalDispatcher,
    },
    util::smart_subscriber::{SmartSubscriber, SubscriberStarter},
};

use super::{
    Client,
    tunnel_manager::{TunnelConfig, TunnelCreateError},
};

#[derive(Clone, Debug)]
pub enum RemoteData<T> {
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
    client: Arc<Client>,
    connection: ForwardConnection<DirectConnection>,
    item: watch::Sender<AgentListItem>,
    realtime: SmartSubscriber<RealtimeStarter>,
    install_info: SmartSubscriber<InstallInfoStarter>,
}

impl Device {
    pub fn new(
        connection: ForwardConnection<DirectConnection>,
        item: AgentListItem,
        client: Arc<Client>,
    ) -> Self {
        return Self {
            data: Arc::new(DeviceData {
                connection: connection.clone(),
                item: watch::channel(item).0,
                realtime: SmartSubscriber::new(
                    RealtimeStarter {
                        connection: connection.clone(),
                    },
                    client.cancellation_token().clone(),
                ),
                install_info: SmartSubscriber::new(
                    InstallInfoStarter { connection },
                    client.cancellation_token().clone(),
                ),
                client,
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

    pub fn subscribe_realtime(&self) -> watch::Receiver<RemoteData<RealtimeStatus>> {
        self.data.realtime.subscribe()
    }

    pub fn subscribe_install_info(&self) -> watch::Receiver<RemoteData<InstallationInfo>> {
        self.data.install_info.subscribe()
    }

    pub async fn open_tunnel(&self, config: TunnelConfig) -> Result<(), TunnelCreateError> {
        self.data
            .client
            .tunnel_manager()
            .open(self.data.connection.clone(), config)
            .await
    }

    pub fn open_terminal(
        &self,
        initial_size: TerminalSize,
    ) -> (
        mpsc::Sender<TerminalInput>,
        mpsc::Receiver<Result<Vec<u8>, ()>>,
    ) {
        let (input_send, input_recv) = tokio::sync::mpsc::channel(10);
        let (output_send, output_recv) = tokio::sync::mpsc::channel(10);

        let dispatcher = RemoteTerminalDispatcher {
            cancel: self.data.client.cancellation_token().clone(),
            input: input_recv,
            output: output_send.clone(),
            initial_size,
        };

        let connection = self.data.connection.clone();

        self.data.client.background_tasks().spawn(async move {
            if let Err(err) = connection.dispatch(dispatcher).await {
                let _ = output_send.try_send(Err(()));
                tracing::error!("Terminal dispatcher error: {}", err);
            }
        });

        (input_send, output_recv)
    }

    pub async fn check_update(&self, channel: UpdateChannel) -> Result<String> {
        self.data
            .connection
            .dispatch(AvailableVersionDispatcher { channel })
            .await
            .map_err(|err| anyhow!(err))
    }

    pub async fn start_update(&self, channel: UpdateChannel) -> Result<()> {
        self.data
            .connection
            .dispatch(StartUpdateDispatcher { channel })
            .await
            .map_err(|err| anyhow!(err))
    }
}

struct InstallInfoStarter {
    connection: ForwardConnection<DirectConnection>,
}

impl SubscriberStarter for InstallInfoStarter {
    type Item = RemoteData<InstallationInfo>;

    fn default(&self) -> Self::Item {
        RemoteData::Unavailable
    }

    fn start(
        &self,
        send: watch::Sender<Self::Item>,
        _cancel: CancellationToken,
    ) -> impl Future<Output = ()> + Send + 'static {
        let connection = self.connection.clone();
        let _ = send.send(RemoteData::Pending);

        async move {
            let send2 = send.clone();
            if let Err(err) = connection
                .dispatch(InstallationInfoDispatcher { send })
                .await
            {
                let _ = send2.send(RemoteData::Unavailable);
                error!("error while requesting InstallationInfo: {err}");
            }
        }
    }
}

struct RealtimeStarter {
    connection: ForwardConnection<DirectConnection>,
}

impl SubscriberStarter for RealtimeStarter {
    type Item = RemoteData<RealtimeStatus>;

    fn default(&self) -> Self::Item {
        RemoteData::Unavailable
    }

    fn start(
        &self,
        send: watch::Sender<Self::Item>,
        cancel: CancellationToken,
    ) -> impl Future<Output = ()> + Send + 'static {
        let connection = self.connection.clone();
        let _ = send.send(RemoteData::Pending);

        async move {
            let send2 = send.clone();
            if let Err(err) = connection
                .dispatch(SubscribeRealtimeStatus { cancel, send })
                .await
            {
                let _ = send2.send(RemoteData::Unavailable);
                error!("error while requesting InstallationInfo: {err}");
            }
        }
    }
}
