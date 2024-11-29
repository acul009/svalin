use std::{collections::HashMap, sync::Arc};

use iced::{
    advanced::graphics::futures::subscription,
    widget::{column, row, text},
    Task,
};
use svalin::{
    client::{
        tunnel_manager::{Tunnel, TunnelConfig},
        Client,
    },
    shared::commands::agent_list::AgentListItem,
};
use svalin_pki::Certificate;
use uuid::Uuid;

use crate::{ui::screen::SubScreen, util::watch_recipe::WatchRecipe, Element};

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
}

impl From<Message> for super::Message {
    fn from(value: Message) -> Self {
        Self::Tunnel(value)
    }
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

        let tunnels: Vec<(AgentListItem, Vec<(Uuid, TunnelConfig)>)> =
            Self::copy_tunnels(client.clone());

        Self {
            client,
            recipe,
            tunnels,
        }
    }

    pub fn copy_tunnels(client: Arc<Client>) -> Vec<(AgentListItem, Vec<(Uuid, TunnelConfig)>)> {
        client
            .tunnel_manager()
            .tunnels()
            .iter()
            .filter_map(|(certificate, tunnels)| {
                let item = match client.device(certificate) {
                    None => return None,
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
            .collect()
    }

    fn tunnel_display<'a>(config: &TunnelConfig, id: &Uuid) -> Element<'a, Message> {
        match config {
            TunnelConfig::Tcp(config) => row![
                text("TCP"),
                text!("{}", config.local_port),
                text("->"),
                text!("{}", config.remote_host),
            ]
            .into(),
        }
    }
}

impl SubScreen for TunnelUi {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> iced::Task<Self::Message> {
        match message {
            Message::Refresh => {
                self.tunnels = Self::copy_tunnels(self.client.clone());
                Task::none()
            }
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        column(self.tunnels.iter().map(|(item, tunnels)| {
            column![
                text(&item.public_data.name),
                column(
                    tunnels
                        .iter()
                        .map(|(id, config)| { Self::tunnel_display(config, id) })
                ),
            ]
            .into()
        }))
        .into()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        subscription::from_recipe(self.recipe.clone())
    }
}
