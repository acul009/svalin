use device_status::DeviceStatus;
use iced::{widget::text, Task};
use svalin::client::device::Device;

use crate::ui::screen::SubScreen;

mod device_status;

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    Status(device_status::Message),
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        Self::DeviceView(message)
    }
}

pub struct DeviceView {
    device: Device,
    status: DeviceStatus,
}

impl DeviceView {
    pub fn start(device: Device) -> (Self, Task<Message>) {
        let (status, task) = DeviceStatus::start(device.clone());
        (Self { device, status }, task.map(Into::into))
    }
}

impl SubScreen for DeviceView {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Status(message) => self.status.update(message).map(Into::into),
            Message::Back => unreachable!(),
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        self.status.view().map(Into::into)
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        self.status.subscription().map(Into::into)
    }
}
