use device_status::DeviceStatus;
use iced::{widget::text, Length, Task};
use svalin::client::device::Device;

use crate::ui::{screen::SubScreen, widgets::scaffold};

mod device_status;
mod remote_terminal;

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    Status(device_status::Message),
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        match message {
            Message::Back => Self::ShowList,
            message => Self::DeviceView(message),
        }
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
        scaffold(self.status.view().map(Into::into))
            .on_back(Message::Back)
            .header(
                text(self.device.item().public_data.name)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center(),
            )
            .into()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        self.status.subscription().map(Into::into)
    }
}
