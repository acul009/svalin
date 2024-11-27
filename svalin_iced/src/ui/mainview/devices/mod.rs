use std::{collections::BTreeMap, sync::Arc};

use add_device::AddDevice;
use device_view::DeviceView;
use iced::{
    advanced::subscription::from_recipe,
    alignment::Vertical,
    widget::{button, column, container, row, stack, text},
    Color, Length, Subscription, Task,
};
use svalin::client::{device::Device, Client};
use svalin_pki::Certificate;

use crate::{
    fl,
    ui::{screen::SubScreen, MapOpt},
    util::watch_recipe::WatchRecipe,
};

mod add_device;
mod device_view;

#[derive(Debug, Clone)]
pub enum Message {
    AddDevice(add_device::Message),
    NewDevice,
    DeviceView(device_view::Message),
    ShowDevice(Device),
    ShowList,
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
    recipe: WatchRecipe<String, BTreeMap<Certificate, Device>, Message>,
}

impl Devices {
    pub fn start(client: Arc<Client>) -> (Self, Task<Message>) {
        let recipe = WatchRecipe::new(
            String::from("devices"),
            client.watch_device_list(),
            Message::ShowList,
        );

        (
            Self {
                client,
                state: State::List,
                recipe,
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
            Message::ShowList => {
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
                        let device = device.1.clone();
                        let name = device.item().public_data.name.clone();
                        let color = match device.item().online_status {
                            true => Color::from_rgb8(0, 255, 0),
                            false => Color::from_rgb8(255, 0, 0),
                        };

                        col.push(
                            button(
                                row![
                                    text("X")
                                        .width(50)
                                        .height(Length::Fill)
                                        .style(move |_| { text::Style { color: Some(color) } })
                                        .center(),
                                    text(name).height(Length::Fill).align_y(Vertical::Center)
                                ]
                                .width(Length::Fill)
                                .height(Length::Fill),
                            )
                            .on_press(Message::ShowDevice(device))
                            .height(50)
                            .width(Length::Fill),
                        )
                    })
            ]
            .into(),
        }
    }

    fn header(&self) -> Option<crate::Element<Self::Message>> {
        match &self.state {
            State::AddDevice(add_device) => add_device.header().mapopt(Into::into),
            State::DeviceView(device_view) => device_view.header().mapopt(Into::into),
            State::List => None,
        }
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        match &self.state {
            State::AddDevice(add_device) => add_device.subscription().map(Into::into),
            State::DeviceView(device_view) => device_view.subscription().map(Into::into),
            State::List => from_recipe(self.recipe.clone()),
        }
    }
}
