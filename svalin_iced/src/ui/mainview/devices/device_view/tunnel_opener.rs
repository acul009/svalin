use std::{fmt::Display, sync::Arc, vec};

use iced::{
    widget::{button, combo_box, container, row, text_input},
    Task,
};
use iced_aw::{card, number_input};
use svalin::client::{
    device::Device,
    tunnel_manager::{tcp::TcpTunnelConfig, TunnelConfig, TunnelCreateError},
};

use crate::ui::{screen::SubScreen, types::error_display_info::ErrorDisplayInfo};

#[derive(Debug, Clone)]
pub enum Message {
    TunnelType(TunnelType),
    LocalPort(u16),
    RemoteHost(String),
    Error(ErrorDisplayInfo<Arc<TunnelCreateError>>),
    CloseError,
    OpenTunnel,
    Noop,
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        Self::TunnelOpener(message)
    }
}

impl From<&TunnelConfig> for &TunnelType {
    fn from(value: &TunnelConfig) -> Self {
        match value {
            TunnelConfig::Tcp(_) => &TunnelType::TCP,
        }
    }
}

impl From<&TunnelConfig> for TunnelType {
    fn from(value: &TunnelConfig) -> Self {
        match value {
            TunnelConfig::Tcp(_) => TunnelType::TCP,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelType {
    TCP,
}

impl Display for TunnelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TunnelType::TCP => "TCP",
        })
    }
}

pub struct TunnelOpener {
    device: Device,
    tunnel_types: combo_box::State<TunnelType>,
    config: Option<TunnelConfig>,
    error: Option<ErrorDisplayInfo<Arc<TunnelCreateError>>>,
}

impl TunnelOpener {
    pub fn new(device: Device) -> Self {
        let config = TunnelConfig::Tcp(TcpTunnelConfig {
            local_port: 8080,
            remote_host: String::new(),
        });

        Self {
            device,
            tunnel_types: combo_box::State::new(vec![TunnelType::TCP]),
            config: Some(config),
            error: None,
        }
    }
}

impl SubScreen for TunnelOpener {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> iced::Task<Self::Message> {
        match message {
            Message::TunnelType(tunnel_type) => {
                if let Some(config) = &self.config {
                    if tunnel_type == config.into() {
                        return Task::none();
                    }
                }
                Task::none()
            }
            Message::LocalPort(port) => {
                match &mut self.config {
                    Some(TunnelConfig::Tcp(config)) => {
                        config.local_port = port;
                    }
                    None => (),
                }
                Task::none()
            }
            Message::RemoteHost(host) => {
                match &mut self.config {
                    Some(TunnelConfig::Tcp(config)) => {
                        config.remote_host = host;
                    }
                    None => (),
                }
                Task::none()
            }
            Message::OpenTunnel => {
                if let Some(config) = &self.config {
                    let device = self.device.clone();
                    let config = config.clone();
                    Task::future(async move {
                        match device.open_tunnel(config).await {
                            Ok(()) => Message::Noop,
                            Err(err) => {
                                Message::Error(ErrorDisplayInfo::new(Arc::new(err), "TODO"))
                            }
                        }
                    })
                } else {
                    Task::none()
                }
            }
            Message::Error(info) => {
                self.error = Some(info);
                Task::none()
            }
            Message::CloseError => {
                self.error = None;
                Task::none()
            }
            Message::Noop => Task::none(),
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        container(card(
            "TODO",
            row![
                combo_box(
                    &self.tunnel_types,
                    "",
                    self.config.as_ref().map(Into::into),
                    Message::TunnelType
                )
                .width(200),
                match &self.config {
                    None => row![],
                    Some(TunnelConfig::Tcp(config)) => {
                        row![
                            number_input(config.local_port, 1..=65535, Message::LocalPort),
                            text_input("TODO", &config.remote_host).on_input(Message::RemoteHost),
                            button("TODO").on_press_maybe(if config.remote_host.is_empty() {
                                None
                            } else {
                                Some(Message::OpenTunnel)
                            })
                        ]
                    }
                }
                .spacing(10),
            ]
            .padding(10)
            .spacing(10),
        ))
        .padding(30)
        .into()
    }

    fn dialog(&self) -> Option<crate::Element<Self::Message>> {
        self.error
            .as_ref()
            .map(|error| error.view().on_close(Message::CloseError).into())
    }
}
