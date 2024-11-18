use std::sync::Arc;

use add_device::AddDevice;
use device_view::DeviceView;
use iced::{
    alignment::Vertical,
    widget::{button, column, container, row, stack, text},
    Color, Length, Subscription, Task,
};
use svalin::client::{device::Device, Client};

use crate::{fl, ui::screen::SubScreen};

pub mod add_device;
pub mod device_view;

#[derive(Debug, Clone)]
pub enum Message {
    AddDevice(add_device::Message),
    NewDevice,
    DeviceView(device_view::Message),
    ShowDevice(Device),
    Reset,
}

impl From<Message> for super::Message {
    fn from(value: Message) -> Self {
        Self::Devices(value)
    }
}

pub enum State {
    List,
    AddDevice(AddDevice),
    DeviceView(DeviceView),
}

pub struct Devices {
    client: Arc<Client>,
    state: State,
}

impl Devices {
    pub fn start(client: Arc<Client>) -> (Self, Task<Message>) {
        (
            Self {
                client,
                state: State::List,
            },
            Task::none(),
        )
    }
}

impl SubScreen for Devices {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::AddDevice(message) => match &mut self.state {
                State::AddDevice(add_device) => add_device.update(message).map(Into::into),
                _ => Task::none(),
            },
            Message::NewDevice => {
                let (add_device, task) = AddDevice::start(self.client.clone());

                self.state = State::AddDevice(add_device);
                task.map(Into::into)
            }
            Message::DeviceView(message) => match &mut self.state {
                State::DeviceView(device_view) => device_view.update(message).map(Into::into),
                _ => Task::none(),
            },
            Message::ShowDevice(device) => {
                let (device_view, task) = DeviceView::start(device);

                self.state = State::DeviceView(device_view);
                task.map(Into::into)
            }
            Message::Reset => {
                self.state = State::List;
                Task::none()
            }
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        match &self.state {
            State::AddDevice(add_device) => add_device.view().map(Into::into),
            State::DeviceView(device_view) => device_view.view().map(Into::into),
            State::List => stack![
                container(
                    button(text(fl!("device-add")))
                        .padding(10)
                        .on_press(Message::NewDevice),
                )
                .align_bottom(Length::Fill)
                .align_right(Length::Fill)
                .padding(30),
                self.client
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
                            .on_press(Message::ShowDevice(device.clone()))
                            .height(50)
                            .width(Length::Fill),
                        )
                    })
            ]
            .into(),
        }
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        match &self.state {
            State::AddDevice(add_device) => add_device.subscription().map(Into::into),
            State::DeviceView(device_view) => device_view.subscription().map(Into::into),
            State::List => Subscription::none(),
        }
    }
}
