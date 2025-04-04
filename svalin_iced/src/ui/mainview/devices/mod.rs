use std::{collections::BTreeMap, sync::Arc};

use add_device::AddDevice;
use device_view::DeviceView;
use iced::{
    Color, Length, Subscription, Task,
    advanced::subscription::from_recipe,
    alignment::Vertical,
    widget::{button, column, container, row, stack, text},
};
use svalin::client::{Client, device::Device};
use svalin_pki::Certificate;

use crate::{ui::MapOpt, util::watch_recipe::WatchRecipe};

mod add_device;
mod device_view;

#[derive(Debug, Clone)]
pub enum Message {
    AddDevice(add_device::Message),
    NewDevice,
    DeviceView(device_view::Message),
    ShowDevice(Device),
    ShowList,
    OpenTunnelGui,
}

pub enum Action {
    None,
    OpenTunnelGui,
    Run(Task<Message>),
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
    pub fn new(client: Arc<Client>) -> Self {
        let recipe = WatchRecipe::new(
            String::from("devices"),
            client.watch_device_list(),
            Message::ShowList,
        );

        Self {
            client,
            state: State::List,
            recipe,
        }
    }
}

impl Devices {
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::AddDevice(message) => match &mut self.state {
                State::AddDevice(add_device) => {
                    let action = add_device.update(message);

                    match action {
                        add_device::Action::Exit => {
                            self.state = State::List;
                            Action::None
                        }
                        add_device::Action::Run(task) => Action::Run(task.map(Message::AddDevice)),
                        add_device::Action::None => Action::None,
                    }
                }
                _ => Action::None,
            },
            Message::NewDevice => {
                let (add_device, task) = AddDevice::start(self.client.clone());

                self.state = State::AddDevice(add_device);
                Action::Run(task.map(Message::AddDevice))
            }
            Message::DeviceView(message) => match &mut self.state {
                State::DeviceView(device_view) => {
                    let action = device_view.update(message);

                    match action {
                        device_view::Action::Back => {
                            self.state = State::List;
                            Action::None
                        }
                        device_view::Action::OpenTunnelGui => Action::OpenTunnelGui,
                        device_view::Action::Run(task) => {
                            Action::Run(task.map(Message::DeviceView))
                        }
                        device_view::Action::None => Action::None,
                    }
                }
                _ => Action::None,
            },
            Message::ShowDevice(device) => {
                let device_view = DeviceView::new(device);

                self.state = State::DeviceView(device_view);
                Action::None
            }
            Message::ShowList => {
                self.state = State::List;
                Action::None
            }
            Message::OpenTunnelGui => Action::OpenTunnelGui,
        }
    }

    pub fn view(&self) -> crate::Element<Message> {
        match &self.state {
            State::AddDevice(add_device) => add_device.view().map(Message::AddDevice),
            State::DeviceView(device_view) => device_view.view().map(Message::DeviceView),
            State::List => stack![
                container(
                    button(text(t!("device_list.add")))
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
                        let color = match device.item().is_online {
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

    pub fn header(&self) -> Option<crate::Element<Message>> {
        match &self.state {
            State::AddDevice(_) => None,
            State::DeviceView(device_view) => device_view.header().mapopt(Message::DeviceView),
            State::List => None,
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        match &self.state {
            State::AddDevice(_) => Subscription::none(),
            State::DeviceView(device_view) => device_view.subscription().map(Message::DeviceView),
            State::List => from_recipe(self.recipe.clone()),
        }
    }

    pub fn dialog(&self) -> Option<crate::Element<Message>> {
        match &self.state {
            State::AddDevice(_) => None,
            State::DeviceView(device_view) => device_view.dialog().mapopt(Message::DeviceView),
            State::List => None,
        }
    }
}
