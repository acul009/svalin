use std::{collections::BTreeMap, sync::Arc};

use add_device::AddDevice;
use device_view::DeviceView;
use iced::{
    Color, Length, Task,
    advanced::subscription::from_recipe,
    alignment::Vertical,
    widget::{button, column, container, row, stack, text},
};
use svalin::client::{Client, device::Device};
use svalin_pki::Certificate;

use crate::{
    ui::{MapOpt, action::Action, screen::SubScreen},
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
    OpenTunnelGui,
}

pub enum Instruction {
    OpenTunnelGui,
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
    type Instruction = Instruction;
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Action<Instruction, Message> {
        match message {
            Message::AddDevice(message) => match &mut self.state {
                State::AddDevice(add_device) => {
                    let action = add_device.update(message).map(Message::AddDevice);

                    match action.instruction {
                        Some(add_device::Instruction::Exit) => {
                            self.state = State::List;
                        }
                        None => (),
                    };

                    action.strip_instruction()
                }
                _ => Action::none(),
            },
            Message::NewDevice => {
                let (add_device, task) = AddDevice::start(self.client.clone());

                self.state = State::AddDevice(add_device);
                task.map(Message::AddDevice).into()
            }
            Message::DeviceView(message) => match &mut self.state {
                State::DeviceView(device_view) => {
                    let action = device_view.update(message).map(Message::DeviceView);

                    match action.instruction {
                        Some(device_view::Instruction::Back) => {
                            self.state = State::List;
                            Action::none()
                        }
                        Some(device_view::Instruction::OpenTunnelGui) => {
                            action.with_instruction(Instruction::OpenTunnelGui)
                        }
                        None => Action::none(),
                    }
                }
                _ => Action::none(),
            },
            Message::ShowDevice(device) => {
                let (device_view, task) = DeviceView::start(device);

                self.state = State::DeviceView(device_view);
                task.map(Message::DeviceView).into()
            }
            Message::ShowList => {
                self.state = State::List;
                Action::none()
            }
            Message::OpenTunnelGui => Action::instruction(Instruction::OpenTunnelGui),
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
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
            State::AddDevice(add_device) => add_device.header().mapopt(Message::AddDevice),
            State::DeviceView(device_view) => device_view.header().mapopt(Message::DeviceView),
            State::List => None,
        }
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        match &self.state {
            State::AddDevice(add_device) => add_device.subscription().map(Message::AddDevice),
            State::DeviceView(device_view) => device_view.subscription().map(Message::DeviceView),
            State::List => from_recipe(self.recipe.clone()),
        }
    }

    fn dialog(&self) -> Option<crate::Element<Self::Message>> {
        match &self.state {
            State::AddDevice(add_device) => add_device.dialog().mapopt(Message::AddDevice),
            State::DeviceView(device_view) => device_view.dialog().mapopt(Message::DeviceView),
            State::List => None,
        }
    }
}
