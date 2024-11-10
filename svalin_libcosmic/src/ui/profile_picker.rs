use std::sync::Arc;

use crate::fl;
use cosmic::{
    iced::Length,
    iced_widget::{button, column, row, stack},
    widget::{container, dialog, text, text_input},
    Element, Task,
};
use svalin::client::Client;

use super::widgets::form;

enum State {
    SelectProfile(Vec<String>),
    UnlockProfile {
        profile: String,
        password: String,
    },
    Loading,
    AddProfile {
        host: String,
    },
    RegisterRoot {
        host: String,
        username: String,
        password: String,
        confirm_password: String,
    },
}

pub struct ProfilePicker {
    state: State,
    confirm_delete: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Input(Input),
    Reset,
    SelectProfile(String),
    DeleteProfile(String),
    ConfirmDelete(String),
    CalcelDelete,
    UnlockProfile,
    AddProfile,
    Connect(String),
    Profile(Arc<Client>),
}

#[derive(Debug, Clone)]
pub enum Input {
    Host(String),
    Username(String),
    Password(String),
    ConfirmPassword(String),
}

struct DeleteDialog {
    host: String,
    username: String,
    password: String,
    confirm_password: String,
}

impl ProfilePicker {
    pub fn new() -> Self {
        let profiles = Client::get_profiles().unwrap_or_else(|_| Vec::new());
        Self {
            state: State::SelectProfile(profiles),
            confirm_delete: None,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Reset => {
                let profiles = Client::get_profiles().unwrap_or_else(|_| Vec::new());

                self.state = State::SelectProfile(profiles);
            }
            Message::SelectProfile(profile) => {
                self.state = State::UnlockProfile {
                    profile,
                    password: String::new(),
                };
            }
            Message::DeleteProfile(profile) => {
                self.confirm_delete = Some(profile.clone());
            }
            Message::ConfirmDelete(profile) => {
                self.confirm_delete = None;

                Client::remove_profile(&profile).unwrap();
            }
            Message::CalcelDelete => {
                self.confirm_delete = None;
            }
            Message::UnlockProfile => {
                if let State::UnlockProfile { profile, password } = &self.state {
                    let profile = profile.clone();
                    let password = password.clone();

                    self.state = State::Loading;

                    return Task::future(async move {
                        let client = Client::open_profile_string(profile, password)
                            .await
                            .unwrap();

                        Message::Profile(Arc::new(client))
                    });
                }
            }
            Message::AddProfile => {
                self.state = State::AddProfile {
                    host: String::new(),
                };
            }
            Message::Connect(host) => {
                // TODO

                self.state = State::RegisterRoot {
                    host,
                    username: String::new(),
                    password: String::new(),
                    confirm_password: String::new(),
                };
            }
            Message::Input(input) => match &mut self.state {
                State::AddProfile { host } => match input {
                    Input::Host(new_host) => *host = new_host,
                    _ => unreachable!(),
                },
                State::RegisterRoot {
                    host,
                    username,
                    password,
                    confirm_password,
                } => match input {
                    Input::Host(new_host) => *host = new_host,
                    Input::Username(new_username) => *username = new_username,
                    Input::Password(new_password) => *password = new_password,
                    Input::ConfirmPassword(new_confirm_password) => {
                        *confirm_password = new_confirm_password
                    }
                    _ => unreachable!(),
                },
                _ => {
                    unreachable!()
                }
            },
            Message::Profile(_) => unreachable!(),
        };

        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        match &self.state {
            State::SelectProfile(profiles) => {
                let profiles = column(profiles.iter().map(|p| {
                    row![
                        button(text(p).center())
                            .on_press(Message::SelectProfile(p.clone()))
                            .width(Length::Fill)
                            .height(Length::Fill),
                        button(text("🗑️").center())
                            .on_press(Message::DeleteProfile(p.clone()))
                            .height(Length::Fill)
                    ]
                    .height(60)
                    .into()
                }));

                let overlay = container(
                    button(text(fl!("profile-add")))
                        .padding(10)
                        .on_press(Message::AddProfile),
                )
                .align_bottom(Length::Fill)
                .align_right(Length::Fill)
                .padding(30);

                stack![profiles, overlay]
                    .height(Length::Fill)
                    .width(Length::Fill)
                    .into()
            }
            State::UnlockProfile { profile, password } => column![
                text(fl!("profile-unlock")),
                text_input("Password", password)
                    .password()
                    .on_input(|input| Message::Input(Input::Password(input)))
                    .on_submit(Message::UnlockProfile),
                row![
                    button(text("Cancel")).on_press(Message::Reset),
                    button(text("Unlock")).on_press(Message::UnlockProfile)
                ]
            ]
            .into(),
            State::Loading => text("Loading...").into(),
            State::AddProfile { host } => form()
                .title(fl!("profile-add"))
                .control(
                    text_input("Host", host).on_input(|input| Message::Input(Input::Host(input))),
                )
                .primary_action(
                    button(text(fl!("continue"))).on_press(Message::Connect(host.clone())),
                )
                .secondary_action(button(text(fl!("cancel"))).on_press(Message::Reset))
                .into(),
            State::RegisterRoot {
                host,
                username,
                password,
                confirm_password,
            } => form()
                .title(fl!("profile-add"))
                .control(column![
                    text_input(fl!("username"), username)
                        .on_input(|input| Message::Input(Input::Username(input))),
                    text_input(fl!("password"), password)
                        .on_input(|input| Message::Input(Input::Password(input))),
                    text_input(fl!("confirm-password"), confirm_password)
                        .on_input(|input| Message::Input(Input::ConfirmPassword(input))),
                ])
                .primary_action(button(text(fl!("continue"))).on_press(Message::Reset))
                .secondary_action(button(text(fl!("cancel"))).on_press(Message::Reset))
                .into(),
        }
    }

    pub fn dialog(&self) -> Option<Element<Message>> {
        if let Some(profile) = &self.confirm_delete {
            return Some(
                dialog()
                    .body(format!("Are you sure you want to delete {}", profile))
                    .primary_action(button(text("Cancel")).on_press(Message::CalcelDelete))
                    .secondary_action(
                        button(text("Delete")).on_press(Message::ConfirmDelete(profile.clone())),
                    )
                    .title("Delete Profile")
                    .into(),
            );
        }
        None
    }
}
