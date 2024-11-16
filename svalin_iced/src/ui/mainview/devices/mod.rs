use std::sync::Arc;

use add_device::AddDevice;
use iced::{
    widget::{button, column, container, stack, text},
    Length, Task,
};
use svalin::client::Client;

use crate::{fl, ui::screen::SubScreen};

pub mod add_device;

#[derive(Debug, Clone)]
pub enum Message {
    AddDevice(add_device::Message),
    NewDevice,
    Reset,
}

impl From<Message> for super::Message {
    fn from(value: Message) -> Self {
        Self::Devices(value)
    }
}

pub struct Devices {
    client: Arc<Client>,
    add_device: Option<AddDevice>,
}

impl Devices {
    pub fn start(client: Arc<Client>) -> (Self, Task<Message>) {
        (
            Self {
                client,
                add_device: None,
            },
            Task::none(),
        )
    }
}

impl SubScreen for Devices {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::AddDevice(message) => match &mut self.add_device {
                Some(add_device) => add_device.update(message).map(Into::into),
                None => Task::none(),
            },
            Message::NewDevice => {
                let (state, task) = AddDevice::start(self.client.clone());

                self.add_device = Some(state);
                task.map(Into::into)
            }
            Message::Reset => {
                self.add_device = None;
                Task::none()
            }
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        if let Some(add_device) = &self.add_device {
            return add_device.view().map(Into::into);
        }

        let col = self
            .client
            .device_list()
            .iter()
            .fold(column!(), |col, device| {
                let item = device.item();

                col.push(button(text(item.public_data.cert.spki_hash().to_string())))
            });

        let overlay = container(
            button(text(fl!("device-add")))
                .padding(10)
                .on_press(Message::NewDevice),
        )
        .align_bottom(Length::Fill)
        .align_right(Length::Fill)
        .padding(30);

        stack![overlay, col].into()
    }
}
