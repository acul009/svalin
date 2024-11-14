use std::{ops::Deref, sync::Arc};

use crate::{fl, Element};
use anyhow::Result;
use iced::{
    widget::{button, column, container, image, image::Handle, row, stack, text, text_input},
    Length, Task,
};
use init_server::InitServer;
use svalin::client::{Client, FirstConnect, Init, Login};

use super::{
    screen::SubScreen,
    types::error_display_info::ErrorDisplayInfo,
    widgets::{dialog, error_display, form, loading},
};

mod init_server;

enum State {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    SelectProfile(Vec<String>),
    UnlockProfile { profile: String, password: String },
    Loading(String),
    AddProfile { host: String },
    InitServer(InitServer),
}

pub struct ProfilePicker {
    state: State,
    confirm_delete: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    Input(Input),
    Reset,
    SelectProfile(String),
    DeleteProfile(String),
    ConfirmDelete(String),
    CalcelDelete,
    UnlockProfile,
    AddProfile(Option<String>),
    Connect(String),
    Init(Arc<Init>),
    InitServer(init_server::Message),
    Login(Arc<Login>),
    Profile(Arc<Client>),
}

#[derive(Debug, Clone)]
pub enum Input {
    Host(String),
    Password(String),
}

impl Input {
    fn update(self, state: &mut ProfilePicker) {
        match &mut state.state {
            State::AddProfile { host } => {
                if let Input::Host(new_host) = self {
                    *host = new_host;
                }
            }
            State::UnlockProfile { profile, password } => match self {
                Self::Password(new_password) => {
                    *password = new_password;
                }
                _ => (),
            },
            _ => (),
        }
    }
}

impl ProfilePicker {
    pub fn new() -> Self {
        match Client::get_profiles() {
            Ok(profiles) => Self {
                state: State::SelectProfile(profiles),
                confirm_delete: None,
            },
            Err(err) => Self {
                state: State::Error(ErrorDisplayInfo::new(
                    Arc::new(err),
                    fl!("profile-list-error"),
                )),
                confirm_delete: None,
            },
        }
    }
}

impl SubScreen for ProfilePicker {
    type Message = Message;

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::InitServer(init_server::Message::Exit(host)) => {
                self.state = State::AddProfile { host };
            }
            Message::InitServer(message) => {
                if let State::InitServer(init_server) = &mut self.state {
                    return init_server.update(message).map(Message::InitServer);
                }
            }
            Message::Error(display_info) => self.state = State::Error(display_info),
            Message::Input(input) => input.update(self),
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

                if let Err(error) = Client::remove_profile(&profile) {
                    self.state = State::Error(ErrorDisplayInfo::new(
                        Arc::new(error),
                        fl!("profile-delete-error"),
                    ));
                }
            }
            Message::CalcelDelete => {
                self.confirm_delete = None;
            }
            Message::UnlockProfile => {
                if let State::UnlockProfile { profile, password } = &self.state {
                    let profile = profile.clone();
                    let password = password.clone();

                    self.state = State::Loading(fl!("profile-unlock"));

                    return Task::future(async move {
                        match Client::open_profile_string(profile, password).await {
                            Ok(client) => Message::Profile(Arc::new(client)),
                            Err(err) => Message::Error(ErrorDisplayInfo::new(
                                Arc::new(err),
                                fl!("profile-unlock-error"),
                            )),
                        }
                    });
                }
            }
            Message::AddProfile(host) => {
                self.state = State::AddProfile {
                    host: host.unwrap_or_default(),
                };
            }
            Message::Connect(host) => {
                self.state = State::Loading(fl!("connect-to-server"));
                return Task::future(async move {
                    let connected = Client::first_connect(host).await;

                    match connected {
                        Ok(FirstConnect::Init(init)) => Message::Init(Arc::new(init)),
                        Ok(FirstConnect::Login(login)) => Message::Login(Arc::new(login)),
                        Err(e) => Message::Error(ErrorDisplayInfo::new(
                            Arc::new(e),
                            fl!("connect-to-server-error"),
                        )),
                    }
                });
            }
            Message::Init(init) => {
                self.state = State::InitServer(InitServer::new(init));
            }
            Message::Login(login) => {
                todo!()
            }
            Message::Profile(_) => unreachable!(),
        };

        Task::none()
    }

    fn view(&self) -> Element<Message> {
        match &self.state {
            State::InitServer(init_server) => init_server.view().map(Message::InitServer),
            State::Error(display_info) => display_info.view().on_close(Message::Reset).into(),
            State::Loading(message) => loading(message).expand().into(),
            State::SelectProfile(profiles) => {
                let profiles = column(profiles.iter().map(|p| {
                    row![
                        button(text(p))
                            .on_press(Message::SelectProfile(p.clone()))
                            .width(Length::Fill)
                            .height(Length::Fill),
                        button(text("🗑️").center())
                            .on_press(Message::DeleteProfile(p.clone()))
                            .width(50)
                            .height(Length::Fill)
                    ]
                    .height(60)
                    .into()
                }));

                let overlay = container(
                    button(text(fl!("profile-add")))
                        .padding(10)
                        .on_press(Message::AddProfile(None)),
                )
                .align_bottom(Length::Fill)
                .align_right(Length::Fill)
                .padding(30);

                stack![profiles, overlay]
                    .height(Length::Fill)
                    .width(Length::Fill)
                    .into()
            }
            State::UnlockProfile {
                profile: _,
                password,
            } => form()
                .title(fl!("profile-unlock"))
                .control(
                    text_input("Password", password)
                        .secure(true)
                        .on_input(|input| Message::Input(Input::Password(input)))
                        .on_submit(Message::UnlockProfile),
                )
                .primary_action(button(text(fl!("unlock"))).on_press(Message::UnlockProfile))
                .secondary_action(button(text(fl!("cancel"))).on_press(Message::Reset))
                .into(),
            State::AddProfile { host } => form()
                .title(fl!("profile-add"))
                .control(
                    text_input("Host", host)
                        .on_input(|input| Message::Input(Input::Host(input)))
                        .on_submit(Message::Connect(host.clone())),
                )
                .primary_action(
                    button(text(fl!("continue"))).on_press(Message::Connect(host.clone())),
                )
                .secondary_action(button(text(fl!("cancel"))).on_press(Message::Reset))
                .into(),
        }
    }

    fn dialog(&self) -> Option<Element<Message>> {
        if let Some(profile) = &self.confirm_delete {
            return Some(
                dialog()
                    .body(fl!("confirm-delete", name = profile))
                    // .body(format!("Are you sure you want to delete {}", profile))
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
