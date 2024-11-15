use std::sync::Arc;

use iced::{
    widget::{button, column, stack, text},
    Task,
};
use svalin::{
    client::{device::Device, Client},
    shared::commands::agent_list::AgentListItem,
};

use crate::ui::screen::SubScreen;

#[derive(Debug, Clone)]
pub enum Message {}

impl From<Message> for super::Message {
    fn from(value: Message) -> Self {
        Self::Devices(value)
    }
}

pub struct Devices {
    client: Arc<Client>,
    devices: Vec<Device>,
}

impl Devices {
    pub fn start(client: Arc<Client>) -> (Self, Task<Message>) {
        (
            Self {
                client,
                devices: Vec::new(),
            },
            Task::none(),
        )
    }
}

impl SubScreen for Devices {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        todo!()
    }

    fn view(&self) -> crate::Element<Self::Message> {
        let devices: Vec<AgentListItem> = self
            .client
            .device_list()
            .into_iter()
            .map(|device| device.item())
            .collect();

        let col = self.devices.iter().fold(column!(), |col, device| {
            let item = device.item();

            col.push(button(text(item.public_data.cert.spki_hash())))
        });

        stack![col].into()
    }
}
