use device_status::DeviceStatus;
use iced::{
    Length, Subscription, Task,
    advanced::subscription::from_recipe,
    widget::{column, scrollable, text},
};
use svalin::{client::device::Device, shared::commands::agent_list::AgentListItem};
use tunnel_opener::TunnelOpener;

use crate::{
    ui::{MapOpt, action::Action, screen::SubScreen, widgets::header},
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

pub enum Instruction {
    Back,
    OpenTunnelGui,
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
            task.map(Message::Status),
        )
    }
}

impl SubScreen for DeviceView {
    type Instruction = Instruction;
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Action<Instruction, Message> {
        match message {
            Message::Status(message) => self
                .status
                .update(message)
                .map(Message::Status)
                .strip_instruction(),
            Message::TunnelOpener(message) => self
                .tunnel_opener
                .update(message)
                .map(Message::TunnelOpener)
                .map_instruction(|instuction| match instuction {
                    tunnel_opener::Instruction::OpenTunnelGui => Instruction::OpenTunnelGui,
                }),
            Message::ItemUpdate => {
                self.item = self.device.item().clone();
                Action::none()
            }
            Message::Back => Action::instruction(Instruction::Back),
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        scrollable(
            column![
                self.status.view().map(Message::Status),
                self.tunnel_opener.view().map(Message::TunnelOpener),
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
        self.tunnel_opener.dialog().mapopt(Message::TunnelOpener)
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        Subscription::batch(vec![
            self.status.subscription().map(Message::Status),
            from_recipe(self.recipe.clone()),
        ])
    }
}
