use iced::{
    Task,
    advanced::subscription::from_recipe,
    widget::{button, center, column, combo_box, container, horizontal_rule, row, text},
};
use iced_aw::card;
use svalin::{
    agent::update::{InstallationInfo, UpdateChannel},
    client::device::{Device, RemoteLiveData},
};

use crate::{Element, ui::widgets::loading, util::watch_recipe::WatchRecipe};

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
    Channel(UpdateChannel),
    StartUpdate,
    PossibleUpdate(Option<String>),
}

pub enum Action {
    None,
    Run(Task<Message>),
}

enum PossibleUpdate {
    None,
    Loading,
    Version(String),
}

pub struct UpdateInstaller {
    device: Device,
    data: RemoteLiveData<InstallationInfo>,
    recipe: WatchRecipe<String, RemoteLiveData<InstallationInfo>, Message>,
    possible_update: PossibleUpdate,
    channels: combo_box::State<UpdateChannel>,
    selected_channel: Option<UpdateChannel>,
}

impl UpdateInstaller {
    pub fn start(device: Device) -> Self {
        let recipe = WatchRecipe::new(
            format!(
                "install-info-{:x?}",
                device.item().public_data.cert.fingerprint()
            ),
            device.subscribe_install_info(),
            Message::Refresh,
        );

        let update_installer = Self {
            device,
            data: RemoteLiveData::Pending,
            recipe,
            channels: combo_box::State::new(vec![UpdateChannel::Alpha]),
            selected_channel: None,
            possible_update: PossibleUpdate::None,
        };
        update_installer
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Refresh => {
                self.data = self.recipe.borrow().clone();

                Action::None
            }
            Message::Channel(channel) => {
                self.selected_channel = Some(channel.clone());

                let device = self.device.clone();
                self.possible_update = PossibleUpdate::Loading;

                Action::Run(Task::future(async move {
                    let version = device.check_update(channel).await;
                    Message::PossibleUpdate(version.ok())
                }))
            }
            Message::PossibleUpdate(possible_update) => {
                match possible_update {
                    None => self.possible_update = PossibleUpdate::None,
                    Some(version) => self.possible_update = PossibleUpdate::Version(version),
                }

                Action::None
            }
            Message::StartUpdate => {
                // TODO
                Action::None
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let content: Element<Message> = match &self.data {
            RemoteLiveData::Pending => loading(t!("device.update.loading-status"))
                .height(200)
                .into(),
            RemoteLiveData::Unavailable => center(text(t!("device.update.status-unavailable")))
                .height(200)
                .into(),
            RemoteLiveData::Ready(install_info) => {
                let mut col = column![
                    row![
                        container(text(t!("device.update.current-version") + ":")).width(200),
                        text(install_info.current_version.to_string()),
                    ]
                    .spacing(10),
                    row![
                        container(text(t!("device.update.method") + ":")).width(200),
                        text(install_info.install_method.to_string()),
                    ]
                    .spacing(10),
                    horizontal_rule(2),
                ]
                .spacing(10)
                .padding(10);

                if install_info.install_method.supports_update() {
                    col = col
                        .push(
                            row![
                                container(combo_box(
                                    &self.channels,
                                    "",
                                    self.selected_channel.as_ref(),
                                    Message::Channel
                                ))
                                .width(200),
                                button(text(t!("device.update.start"))).on_press_maybe(
                                    self.selected_channel.as_ref().map(|_| Message::StartUpdate)
                                )
                            ]
                            .spacing(10),
                        )
                        .push_maybe(match &self.possible_update {
                            PossibleUpdate::None => None,
                            PossibleUpdate::Loading => {
                                Some(Element::from(container(loading("").height(40))))
                            }
                            PossibleUpdate::Version(version) => Some(Element::from(
                                row![
                                    container(text("device.update.possible-version")).width(200),
                                    text(version.as_str())
                                ]
                                .spacing(10),
                            )),
                        })
                } else {
                    col = col.push(text(t!("device.update.unsupported")))
                }

                col.into()
            }
        };

        container(card(text(t!("device.update.title")), content))
            .padding(30)
            .into()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        from_recipe(self.recipe.clone())
    }
}
