use device_status::DeviceStatus;
use iced::{
    Length, Subscription, Task,
    advanced::subscription::from_recipe,
    widget::{column, scrollable, text},
};
use svalin::{client::device::Device, shared::commands::agent_list::AgentListItem};
use tunnel_opener::TunnelOpener;
use update_installer::UpdateInstaller;

use crate::{
    ui::{MapOpt, widgets::header},
    util::watch_recipe::WatchRecipe,
};

mod device_status;
mod tunnel_opener;
mod update_installer;
// mod remote_terminal;

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    Status(device_status::Message),
    UpdateInstaller(update_installer::Message),
    TunnelOpener(tunnel_opener::Message),
    ItemUpdate,
}

pub enum Action {
    None,
    Back,
    OpenTunnelGui,
    Run(Task<Message>),
}

pub struct DeviceView {
    device: Device,
    item: AgentListItem,
    status: DeviceStatus,
    update_installer: UpdateInstaller,
    tunnel_opener: TunnelOpener,
    recipe: WatchRecipe<String, AgentListItem, Message>,
}

impl DeviceView {
    pub fn new(device: Device) -> DeviceView {
        let item = device.item().clone();
        let status = DeviceStatus::new(&device);

        let recipe = WatchRecipe::new(
            format!("device-{:x?}", item.public_data.cert.fingerprint()),
            device.subscribe_item(),
            Message::ItemUpdate,
        );

        let tunnel_opener = TunnelOpener::new(device.clone());
        let update_installer = UpdateInstaller::start(device.clone());

        DeviceView {
            device,
            item,
            status,
            update_installer,
            recipe,
            tunnel_opener,
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Status(message) => {
                self.status.update(message);

                Action::None
            }
            Message::UpdateInstaller(message) => match self.update_installer.update(message) {
                update_installer::Action::None => Action::None,
                update_installer::Action::Run(task) => {
                    Action::Run(task.map(Message::UpdateInstaller))
                }
            },
            Message::TunnelOpener(message) => {
                let action = self.tunnel_opener.update(message);

                match action {
                    tunnel_opener::Action::OpenTunnelGui => Action::OpenTunnelGui,
                    tunnel_opener::Action::Run(task) => {
                        Action::Run(task.map(Message::TunnelOpener))
                    }
                    tunnel_opener::Action::None => Action::None,
                }
            }
            Message::ItemUpdate => {
                self.item = self.device.item().clone();
                Action::None
            }
            Message::Back => Action::Back,
        }
    }

    pub fn view(&self) -> crate::Element<Message> {
        scrollable(
            column![
                self.status.view().map(Message::Status),
                self.tunnel_opener.view().map(Message::TunnelOpener),
                self.update_installer.view().map(Message::UpdateInstaller),
            ]
            .height(Length::Shrink),
        )
        .into()
    }

    pub fn header(&self) -> Option<crate::Element<Message>> {
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

    pub fn dialog(&self) -> Option<crate::Element<Message>> {
        if let Some(dialog) = self.tunnel_opener.dialog().mapopt(Message::TunnelOpener) {
            Some(dialog)
        } else if let Some(dialog) = self
            .update_installer
            .dialog()
            .mapopt(Message::UpdateInstaller)
        {
            Some(dialog)
        } else {
            None
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        Subscription::batch(vec![
            self.status.subscription().map(Message::Status),
            self.update_installer
                .subscription()
                .map(Message::UpdateInstaller),
            from_recipe(self.recipe.clone()),
        ])
    }
}
