use std::sync::Arc;

use devices::Devices;
use iced::{
    advanced::graphics::futures::subscription,
    widget::{button, row, text},
    Length, Subscription, Task,
};
use svalin::client::Client;
use tunnel::TunnelUi;
use uuid::timestamp::context;

use super::{screen::SubScreen, MapOpt};

mod devices;
mod tunnel;

#[derive(Debug, Clone)]
pub enum Message {
    Devices(devices::Message),
    Tunnel(tunnel::Message),
    Context(Context),
}

impl From<Message> for super::Message {
    fn from(value: Message) -> Self {
        Self::MainView(value)
    }
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
            task.map(Into::into),
        )
    }
}

impl SubScreen for MainView {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Devices(msg) => self.devices.update(msg).map(Into::into),
            Message::Tunnel(msg) => self.tunnel_ui.update(msg).map(Into::into),
            Message::Context(context) => {
                if self.context == context {
                    self.context = Context::None;
                } else {
                    self.context = context;
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        match &self.state {
            State::Devices => self.devices.view().map(Into::into),
        }
    }

    fn header(&self) -> Option<crate::Element<Self::Message>> {
        let subheader = match &self.state {
            State::Devices => self.devices.header().mapopt(Into::into),
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

    fn context(&self) -> Option<crate::Element<Self::Message>> {
        match &self.context {
            Context::None => None,
            Context::Tunnel => Some(self.tunnel_ui.view().map(Into::into)),
            Context::Test => Some(text("test").into()),
        }
    }

    fn dialog(&self) -> Option<crate::Element<Self::Message>> {
        match self.state {
            State::Devices => self.devices.dialog().mapopt(Into::into),
        }
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let state_subscription = match &self.state {
            State::Devices => self.devices.subscription().map(Into::into),
        };

        let context_subscription = match &self.context {
            Context::None => None,
            Context::Tunnel => Some(self.tunnel_ui.subscription().map(Into::into)),
            Context::Test => None,
        };

        let mut subscriptions = vec![state_subscription];

        if let Some(context_subscription) = context_subscription {
            subscriptions.push(context_subscription);
        }

        Subscription::batch(subscriptions)
    }
}
