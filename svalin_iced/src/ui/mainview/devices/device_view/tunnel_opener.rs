use std::{fmt::Display, sync::Arc, vec};

use iced::{
    Task,
    widget::{button, combo_box, container, row, text, text_input},
};
use iced_aw::{card, number_input};
use svalin::client::{
    device::Device,
    tunnel_manager::{TunnelConfig, TunnelCreateError, tcp::TcpTunnelConfig},
};

use crate::ui::types::error_display_info::ErrorDisplayInfo;

#[derive(Debug, Clone)]
pub enum Message {
    TunnelType(TunnelType),
    LocalPort(u16),
    RemoteHost(String),
    Error(ErrorDisplayInfo<Arc<TunnelCreateError>>),
    CloseError,
    OpenTunnel,
    OpenTunnelGui,
}

pub enum Action {
    None,
    OpenTunnelGui,
    Run(Task<Message>),
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
        Self {
            device,
            tunnel_types: combo_box::State::new(vec![TunnelType::TCP]),
            config: None,
            error: None,
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::TunnelType(tunnel_type) => {
                if let Some(config) = &self.config {
                    if tunnel_type == config.into() {
                        return Action::None;
                    }
                }

                self.config = match tunnel_type {
                    TunnelType::TCP => Some(TunnelConfig::Tcp(TcpTunnelConfig {
                        local_port: 8080,
                        remote_host: String::new(),
                    })),
                };

                Action::None
            }
            Message::LocalPort(port) => {
                match &mut self.config {
                    Some(TunnelConfig::Tcp(config)) => {
                        config.local_port = port;
                    }
                    None => (),
                }
                Action::None
            }
            Message::RemoteHost(host) => {
                match &mut self.config {
                    Some(TunnelConfig::Tcp(config)) => {
                        config.remote_host = host;
                    }
                    None => (),
                }
                Action::None
            }
            Message::OpenTunnel => {
                if let Some(config) = &self.config {
                    let device = self.device.clone();
                    let config = config.clone();
                    Action::Run(Task::future(async move {
                        match device.open_tunnel(config).await {
                            Ok(()) => Message::OpenTunnelGui,
                            Err(err) => Message::Error(ErrorDisplayInfo::new(
                                Arc::new(err),
                                t!("tunnel.error"),
                            )),
                        }
                    }))
                } else {
                    Action::None
                }
            }
            Message::Error(info) => {
                self.error = Some(info);
                Action::None
            }
            Message::CloseError => {
                self.error = None;
                Action::None
            }
            Message::OpenTunnelGui => Action::OpenTunnelGui,
        }
    }

    pub fn view(&self) -> crate::Element<Message> {
        container(card(
            text(t!("tunnel.opener.title")),
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
                            text_input(&t!("tunnel.input.remote_host"), &config.remote_host)
                                .on_input(Message::RemoteHost)
                                .on_submit_maybe(if config.remote_host.is_empty() {
                                    None
                                } else {
                                    Some(Message::OpenTunnel)
                                }),
                            button(text(t!("tunnel.open"))).on_press_maybe(
                                if config.remote_host.is_empty() {
                                    None
                                } else {
                                    Some(Message::OpenTunnel)
                                }
                            )
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

    pub fn dialog(&self) -> Option<crate::Element<Message>> {
        self.error
            .as_ref()
            .map(|error| error.view().on_close(Message::CloseError).into())
    }
}
