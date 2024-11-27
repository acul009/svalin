use std::{borrow::Cow, hash::Hash};

use device_status::DeviceStatus;
use futures_util::{FutureExt, SinkExt, StreamExt};
use iced::{
    advanced::subscription::{from_recipe, Recipe},
    stream::channel,
    widget::{shader::wgpu::core::device, text},
    Length, Subscription, Task,
};
use svalin::{client::device::Device, shared::commands::agent_list::AgentListItem};
use tokio::sync::watch;
use tokio_stream::wrappers::{ReceiverStream, WatchStream};

use crate::{
    ui::{
        screen::SubScreen,
        widgets::{header, scaffold},
    },
    util::watch_recipe::WatchRecipe,
};

mod device_status;
// mod remote_terminal;

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    Status(device_status::Message),
    ItemUpdate,
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
    item: AgentListItem,
    status: DeviceStatus,
    recipe: WatchRecipe<String, AgentListItem, Message>,
}

impl DeviceView {
    pub fn start(device: Device) -> (DeviceView, Task<Message>) {
        let item = device.item().clone();
        let (status, task) = DeviceStatus::start(device.clone());

        let recipe = WatchRecipe::new(
            format!("device-{:x?}", item.public_data.cert.fingerprint()),
            device.watch_item(),
            Message::ItemUpdate,
        );

        (
            DeviceView {
                device,
                status,
                item,
                recipe,
            },
            task.map(Into::into),
        )
    }
}

impl SubScreen for DeviceView {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Status(message) => self.status.update(message).map(Into::into),
            Message::ItemUpdate => {
                self.item = self.device.item().clone();
                Task::none()
            }
            Message::Back => unreachable!(),
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        self.status.view().map(Into::into)
    }

    fn header(&self) -> Option<crate::Element<Self::Message>> {
        Some(
            header(
                text(&self.item.public_data.name)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center(),
            )
            .on_back(Message::Back)
            .into(),
        )
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        Subscription::batch(vec![
            self.status.subscription().map(Into::into),
            from_recipe(self.recipe.clone()),
        ])
    }
}
