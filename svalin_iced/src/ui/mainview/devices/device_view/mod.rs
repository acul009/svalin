use iced::{widget::text, Task};
use svalin::client::device::Device;

use crate::ui::screen::SubScreen;

#[derive(Debug, Clone)]
pub enum Message {
    Back,
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        Self::DeviceView(message)
    }
}

pub struct DeviceView {
    device: Device,
}

impl DeviceView {
    pub fn start(device: Device) -> (Self, Task<Message>) {
        (Self { device }, Task::none())
    }
}

impl SubScreen for DeviceView {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        todo!()
    }

    fn view(&self) -> crate::Element<Self::Message> {
        text(self.device.item().public_data.name).into()
    }
}
