use std::sync::Arc;

use iced::{
    Length, Task,
    widget::{Column, column, container, row, text},
};
use iced_aw::card;
use svalin::{
    agent::update::UpdateInfo,
    client::{Client, device::Device},
};

use crate::{Element, ui::widgets::loading};

#[derive(Debug, Clone)]
pub enum Message {
    Error,
    UpdateInfo(UpdateInfo),
}

pub enum Action {
    None,
}

pub enum State {
    Loading,
    Error,
    UpdateInfo(UpdateInfo),
}

pub struct UpdateInstaller {
    device: Device,
    state: State,
}

impl UpdateInstaller {
    pub fn start(device: Device) -> (Self, Task<Message>) {
        let update_installer = Self {
            device,
            state: State::Loading,
        };
        let task = update_installer.update_task();
        (update_installer, task)
    }

    fn update_task(&self) -> Task<Message> {
        let device = self.device.clone();
        return Task::future(async move {
            let update_info = match device.check_for_update().await {
                Ok(update_info) => update_info,
                Err(err) => {
                    println!("{}", err);
                    return Message::Error;
                }
            };

            Message::UpdateInfo(update_info)
        });
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Error => {
                self.state = State::Error;

                Action::None
            }
            Message::UpdateInfo(update_info) => {
                self.state = State::UpdateInfo(update_info);

                Action::None
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let content: Element<Message> = match &self.state {
            State::Loading => loading(t!("device.update.loading-status"))
                .height(200)
                .into(),
            State::Error => text(t!("device.update.error")).into(),
            State::UpdateInfo(update_info) => column![
                row![text(format!(
                    "{}: {}",
                    t!("device.update.current-version"),
                    update_info.current_version
                ))],
                row![text(format!(
                    "{}: {}",
                    t!("device.update.type"),
                    update_info.update_method
                ))],
            ]
            .into(),
        };

        container(card(text(t!("device.update.title")), content))
            .padding(30)
            .into()
    }
}
