use std::{borrow::Cow, sync::Arc};

use iced::{
    Length,
    widget::{button, column, row, scrollable, text},
};
use svalin::client::{Client, state::ClientState, tunnel_manager::TunnelConfig};
use svalin_pki::SpkiHash;
use uuid::Uuid;

use crate::{Element, ui::widgets::card};

#[derive(Debug, Clone)]
pub enum Message {
    SelectDevice(SpkiHash),
    Close(SpkiHash, Uuid),
}

pub enum Action {
    None,
    SelectDevice(SpkiHash),
}

pub fn update(message: Message, client: &Arc<Client>) -> Action {
    match message {
        Message::SelectDevice(device) => Action::SelectDevice(device),
        Message::Close(device, id) => {
            client.device(device).close_tunnel(id);
            Action::None
        }
    }
}

pub fn header<'a, Message: 'static>() -> crate::ui::widgets::header::Header<'a, Message> {
    crate::ui::widgets::header(text(t!("tunnel.menu.title")).size(20)).drawer()
}

pub fn view(state: &ClientState) -> Element<'_, Message> {
    let mut tunnels: Vec<_> = state
        .tunneling()
        .iter()
        .flat_map(|(device, tunnels)| tunnels.iter().map(move |(id, tunnel)| (device, id, tunnel)))
        .collect();
    tunnels.sort_by(|(_, id_a, a), (_, id_b, b)| a.name.cmp(&b.name).then_with(|| id_a.cmp(id_b)));

    let mut content = column![].spacing(20).padding(20);
    if tunnels.is_empty() {
        content = content.push(text(t!("tunnel.no_tunnels")));
    }
    for (device, id, tunnel) in tunnels {
        let name = state
            .persistent()
            .devices()
            .get(device)
            .map(|device| device.name())
            .unwrap_or_else(|| Cow::Owned(device.to_string()));
        let TunnelConfig::Tcp { remote_host, .. } = &tunnel.config;
        content = content.push(
            card(
                column![
                    text(name),
                    column![
                        text(t!("tunnel.menu.local")).size(14),
                        text(format!("127.0.0.1:{}", tunnel.local_port())),
                    ]
                    .spacing(4),
                    column![text(t!("tunnel.menu.remote")).size(14), text(remote_host),].spacing(4),
                    row![
                        button(text(t!("tunnel.close")))
                            .on_press(Message::Close(device.clone(), *id)),
                        button(text(t!("warnings.device.action")))
                            .on_press(Message::SelectDevice(device.clone())),
                    ]
                    .spacing(10),
                ]
                .spacing(12),
            )
            .title(text(&tunnel.name))
            .width(Length::Fill),
        );
    }
    scrollable(content.width(Length::Fill))
        .height(Length::Fill)
        .into()
}
