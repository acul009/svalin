use std::sync::Arc;

use add_device::AddDevice;
use iced::{
    alignment::Vertical,
    widget::{button, column, container, row, stack, text},
    Color, Length, Task,
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

                col.push(
                    button(
                        row![
                            text("X")
                                .width(50)
                                .height(Length::Fill)
                                .style(move |_| {
                                    text::Style {
                                        color: Some(match item.online_status {
                                            true => Color::from_rgb8(0, 255, 0),
                                            false => Color::from_rgb8(255, 0, 0),
                                        }),
                                    }
                                })
                                .center(),
                            text(item.public_data.name)
                                .height(Length::Fill)
                                .align_y(Vertical::Center)
                        ]
                        .width(Length::Fill)
                        .height(Length::Fill),
                    )
                    .height(50)
                    .width(Length::Fill),
                )
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
