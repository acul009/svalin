use std::sync::Arc;

use devices::Devices;
use iced::{
    Length, Subscription, Task,
    widget::{button, row, text},
};
use svalin::client::Client;
use tunnel::TunnelUi;

use super::MapOpt;

mod devices;
mod tunnel;

#[derive(Debug, Clone)]
pub enum Message {
    Devices(devices::Message),
    Tunnel(tunnel::Message),
    Context(Context),
}

pub enum Action {
    None,
    Run(Task<Message>),
}

enum State {
    Devices,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    None,
    Tunnel,
    Test,
}

pub struct MainView {
    state: State,
    context: Context,
    devices: Devices,
    tunnel_ui: TunnelUi,
}

impl MainView {
    pub fn start(client: Arc<Client>) -> (Self, Task<Message>) {
        let (devices, task) = Devices::start(client.clone());

        let tunnel_ui = TunnelUi::new(client.clone());

        (
            Self {
                state: State::Devices,
                context: Context::None,
                devices,
                tunnel_ui,
            },
            task.map(Message::Devices),
        )
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Devices(msg) => {
                let action = self.devices.update(msg);

                match action {
                    devices::Action::None => Action::None,
                    devices::Action::OpenTunnelGui => {
                        self.context = Context::Tunnel;
                        Action::None
                    }
                    devices::Action::Run(task) => Action::Run(task.map(Message::Devices)),
                }
            }
            Message::Tunnel(msg) => {
                self.tunnel_ui.update(msg);
                Action::None
            }
            Message::Context(context) => {
                if self.context == context {
                    self.context = Context::None;
                } else {
                    match context {
                        Context::Tunnel => self.tunnel_ui.refresh(),
                        Context::Test | Context::None => (),
                    };
                    self.context = context;
                }

                Action::None
            }
        }
    }

    pub fn view(&self) -> crate::Element<Message> {
        match &self.state {
            State::Devices => self.devices.view().map(Message::Devices),
        }
    }

    pub fn header(&self) -> Option<crate::Element<Message>> {
        let subheader = match &self.state {
            State::Devices => self.devices.header().mapopt(Message::Devices),
        }
        .unwrap_or_else(|| iced::widget::horizontal_space().into());

        let actions = row![
            button(text("T").center().height(Length::Fill))
                .on_press(Message::Context(Context::Tunnel))
                .height(Length::Fill)
                .width(40),
            button(text("2").center().height(Length::Fill))
                .on_press(Message::Context(Context::Test))
                .height(Length::Fill)
                .width(40),
        ]
        .padding(5)
        .spacing(5);

        Some(row![subheader, actions].into())
    }

    pub fn context(&self) -> Option<crate::Element<Message>> {
        match &self.context {
            Context::None => None,
            Context::Tunnel => Some(self.tunnel_ui.view().map(Message::Tunnel)),
            Context::Test => Some(text("test").into()),
        }
    }

    pub fn dialog(&self) -> Option<crate::Element<Message>> {
        match self.state {
            State::Devices => self.devices.dialog().mapopt(Message::Devices),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        let state_subscription = match &self.state {
            State::Devices => self.devices.subscription().map(Message::Devices),
        };

        let context_subscription = match &self.context {
            Context::None => None,
            Context::Tunnel => Some(self.tunnel_ui.subscription().map(Message::Tunnel)),
            Context::Test => None,
        };

        let mut subscriptions = vec![state_subscription];

        if let Some(context_subscription) = context_subscription {
            subscriptions.push(context_subscription);
        }

        Subscription::batch(subscriptions)
    }
}
