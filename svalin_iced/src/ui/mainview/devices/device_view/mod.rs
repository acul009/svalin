use device_status::DeviceStatus;
use futures_util::{FutureExt, StreamExt};
use iced::{
    advanced::subscription::from_recipe,
    widget::{column, scrollable, text},
    Length, Subscription, Task,
};
use svalin::{client::device::Device, shared::commands::agent_list::AgentListItem};
use tunnel_opener::TunnelOpener;

use crate::{
    ui::{screen::SubScreen, widgets::header, MapOpt},
    util::watch_recipe::WatchRecipe,
};

mod device_status;
mod tunnel_opener;
// mod remote_terminal;

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    Status(device_status::Message),
    TunnelOpener(tunnel_opener::Message),
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
    tunnel_opener: TunnelOpener,
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

        let tunnel_opener = TunnelOpener::new(device.clone());

        (
            DeviceView {
                device,
                status,
                item,
                recipe,
                tunnel_opener,
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
            Message::TunnelOpener(message) => self.tunnel_opener.update(message).map(Into::into),
            Message::ItemUpdate => {
                self.item = self.device.item().clone();
                Task::none()
            }
            Message::Back => unreachable!(),
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        scrollable(
            column![
                self.status.view().map(Into::into),
                self.tunnel_opener.view().map(Into::into),
            ]
            .height(Length::Shrink),
        )
        .into()
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

    fn dialog(&self) -> Option<crate::Element<Self::Message>> {
        self.tunnel_opener.dialog().mapopt(Into::into)
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        Subscription::batch(vec![
            self.status.subscription().map(Into::into),
            from_recipe(self.recipe.clone()),
        ])
    }
}
