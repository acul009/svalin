use std::sync::Arc;

use devices::Devices;
use iced::Task;
use svalin::client::Client;

use super::screen::SubScreen;

pub mod devices;

#[derive(Debug, Clone)]
pub enum Message {
    Devices(devices::Message),
}

impl From<Message> for super::Message {
    fn from(value: Message) -> Self {
        Self::MainView(value)
    }
}

enum State {
    Devices,
}

pub struct MainView {
    client: Arc<Client>,
    state: State,
    devices: Devices,
}

impl MainView {
    pub fn start(client: Arc<Client>) -> (Self, Task<Message>) {
        let (devices, task) = Devices::start(client.clone());

        (
            Self {
                client,
                state: State::Devices,
                devices,
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
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        match &self.state {
            State::Devices => self.devices.view().map(Into::into),
        }
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        match &self.state {
            State::Devices => self.devices.subscription().map(Into::into),
        }
    }
}
