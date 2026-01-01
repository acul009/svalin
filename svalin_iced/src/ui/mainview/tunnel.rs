use std::{collections::HashMap, sync::Arc};

use iced::{
    Color, Length, Padding, Shadow, Vector,
    advanced::graphics::futures::subscription,
    alignment::{Horizontal, Vertical},
    widget::{button, column, container, row, text},
};
use svalin::{
    client::{
        Client,
        tunnel_manager::{Tunnel, TunnelConfig},
    },
    shared::commands::agent_list::AgentListItem,
};
use svalin_pki::Certificate;
use uuid::Uuid;

use crate::{Element, util::watch_recipe::WatchRecipe};

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
    CloseTunnel(Uuid),
}

pub struct TunnelUi {
    client: Arc<Client>,
    recipe: WatchRecipe<String, HashMap<Certificate, HashMap<Uuid, Tunnel>>, Message>,
    tunnels: Vec<(AgentListItem, Vec<(Uuid, TunnelConfig)>)>,
}

impl TunnelUi {
    pub fn new(client: Arc<Client>) -> Self {
        let recipe = WatchRecipe::new(
            "tunnels".into(),
            client.tunnel_manager().watch_tunnels(),
            Message::Refresh,
        );

        let tunnels: Vec<(AgentListItem, Vec<(Uuid, TunnelConfig)>)> = Self::copy_tunnels(&client);

        Self {
            client,
            recipe,
            tunnels,
        }
    }

    fn copy_tunnels(client: &Arc<Client>) -> Vec<(AgentListItem, Vec<(Uuid, TunnelConfig)>)> {
        let tunnels = client
            .tunnel_manager()
            .tunnels()
            .iter()
            .filter_map(|(certificate, tunnels)| {
                let item = match client.device(certificate) {
                    None => {
                        println!("No device for certificate {:x?}", certificate.fingerprint());
                        return None;
                    }
                    Some(device) => device,
                }
                .item()
                .clone();

                let tunnels: Vec<_> = tunnels
                    .iter()
                    .map(|(id, tunnel)| (id.clone(), tunnel.config().clone()))
                    .collect();

                Some((item, tunnels))
            })
            .collect();

        println!("{:?}", client.tunnel_manager().tunnels().len());

        tunnels
    }

    fn tunnel_display<'a>(config: &TunnelConfig, id: &Uuid) -> Element<'a, Message> {
        match config {
            TunnelConfig::Tcp(config) => row![
                text("TCP").width(30),
                text!("{}", config.local_port).width(40),
                text("->").width(20).align_x(Horizontal::Center),
                text!("{}", config.remote_host).width(Length::Fill),
                button(text(t!("tunnel.close"))).on_press(Message::CloseTunnel(id.clone()))
            ]
            .padding(10)
            .spacing(20)
            .width(Length::Fill)
            .align_y(Vertical::Center)
            .into(),
        }
    }

    pub fn refresh(&mut self) {
        self.tunnels = Self::copy_tunnels(&self.client);
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Refresh => {
                self.refresh();
            }
            Message::CloseTunnel(id) => {
                self.client.tunnel_manager().close_tunnel(&id);
            }
        }
    }

    pub fn view(&self) -> crate::Element<Message> {
        if self.tunnels.is_empty() {
            return container(text(t!("tunnel.no_tunnels"))).padding(20).into();
        }
        column(self.tunnels.iter().map(|(item, tunnels)| {
            column![
                container(text(&item.certificate.name))
                    .padding(20)
                    .width(Length::Fill)
                    .style(|_| container::Style {
                        shadow: Shadow {
                            color: Color::BLACK,
                            offset: Vector { x: 0.0, y: 10.0 },
                            blur_radius: 20.0,
                        },
                        ..Default::default()
                    }),
                column(
                    tunnels
                        .iter()
                        .map(|(id, config)| { Self::tunnel_display(config, id) })
                )
                .padding(Padding::new(20.0).left(30.0))
                .width(Length::Fill),
            ]
            .width(Length::Fill)
            .into()
        }))
        .width(Length::Fill)
        .into()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        subscription::from_recipe(self.recipe.clone())
    }
}
