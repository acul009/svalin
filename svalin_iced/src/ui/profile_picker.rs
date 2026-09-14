use std::sync::Arc;

use crate::{Element, bootstrap, ui::widgets::dialog};
use iced::{
    Length, Task,
    alignment::{Horizontal, Vertical},
    widget::{button, column, container, opaque, row, scrollable, space, stack, text, text_input},
};
use init_server::InitServer;
use login::LoginDialog;
use svalin::client::{Client, FirstConnect, Init, Login};
use tokio_util::sync::CancellationToken;

use super::{
    types::error_display_info::ErrorDisplayInfo,
    widgets::{button_list, loading},
};

mod init_server;
mod login;

enum Screen {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    SelectProfile,
    UnlockProfile { profile: String, password: String },
    Loading(String),
    AddProfile { host: String },
    InitServer(InitServer),
    LoginDialog(LoginDialog),
    ConfirmDelete(String),
}

pub struct ProfilePicker {
    screen: Screen,
    profiles: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    Input(Input),
    Reset,
    SelectProfile(String),
    DeleteProfile(String),
    ConfirmDelete(String),
    UnlockProfile,
    AddProfile(String),
    Connect(String),
    Init(Arc<Init>),
    InitServer(init_server::Message),
    LoginDialog(login::Message),
    Login(Arc<Login>),
    Profile(Arc<Client>),
    Profiles(Vec<String>),
    None,
}

pub enum Action {
    None,
    OpenProfile(Arc<Client>),
    Run(Task<Message>),
}

#[derive(Debug, Clone)]
pub enum Input {
    Host(String),
    Password(String),
}

impl Input {
    fn update(self, state: &mut ProfilePicker) {
        match &mut state.screen {
            Screen::AddProfile { host } => {
                if let Input::Host(new_host) = self {
                    *host = new_host;
                }
            }
            Screen::UnlockProfile { password, .. } => {
                if let Self::Password(new_password) = self {
                    *password = new_password;
                }
            }
            _ => (),
        }
    }
}

impl ProfilePicker {
    pub fn start() -> (Self, Task<Message>) {
        (
            Self {
                screen: Screen::SelectProfile,
                profiles: Vec::new(),
            },
            Task::done(Message::Reset),
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::None => Action::None,
            Message::InitServer(message) => {
                if let Screen::InitServer(init_server) = &mut self.screen {
                    let action = init_server.update(message);

                    match action {
                        init_server::Action::None => Action::None,
                        init_server::Action::OpenProfile(client) => Action::OpenProfile(client),
                        init_server::Action::Exit(host) => {
                            self.add_profile(host);
                            Action::None
                        }
                        init_server::Action::Run(task) => {
                            Action::Run(task.map(Message::InitServer))
                        }
                    }
                } else {
                    Action::None
                }
            }
            Message::LoginDialog(message) => {
                if let Screen::LoginDialog(login_dialog) = &mut self.screen {
                    let action = login_dialog.update(message);

                    match action {
                        login::Action::None => Action::None,
                        login::Action::Exit(host) => {
                            self.add_profile(host);
                            Action::None
                        }
                        login::Action::OpenProfile(client) => Action::OpenProfile(client),
                        login::Action::Run(task) => Action::Run(task.map(Message::LoginDialog)),
                    }
                } else {
                    Action::None
                }
            }
            Message::Error(display_info) => {
                self.screen = Screen::Error(display_info);
                Action::None
            }
            Message::Input(input) => {
                input.update(self);

                Action::None
            }
            Message::Reset => Action::Run(Task::future(async move {
                match Client::list_profiles().await {
                    Ok(profiles) => Message::Profiles(profiles),
                    Err(err) => Message::Error(ErrorDisplayInfo::new(
                        Arc::new(err),
                        t!("profile-picker.error.list"),
                    )),
                }
            })),
            Message::Profiles(profiles) => {
                self.profiles = profiles;
                if self.profiles.is_empty() {
                    self.add_profile(String::new());
                } else {
                    self.screen = Screen::SelectProfile;
                }

                Action::None
            }
            Message::SelectProfile(profile) => {
                self.screen = Screen::UnlockProfile {
                    profile,
                    password: String::new(),
                };

                Action::Run(iced::widget::operation::focus("password"))
            }
            Message::DeleteProfile(profile) => {
                self.screen = Screen::ConfirmDelete(profile);
                Action::None
            }
            Message::ConfirmDelete(profile) => Action::Run(Task::future(async move {
                if let Err(error) = Client::remove_profile(&profile).await {
                    Message::Error(ErrorDisplayInfo::new(
                        Arc::new(error),
                        t!("profile-picker.error.delete"),
                    ))
                } else {
                    Message::Reset
                }
            })),
            Message::UnlockProfile => {
                if let Screen::UnlockProfile { profile, password } = &self.screen {
                    let profile = profile.clone();
                    let password = password.clone();

                    self.screen = Screen::Loading(t!("profile-picker.unlocking").to_string());

                    Action::Run(Task::future(async move {
                        match Client::open_profile(
                            &profile,
                            password.into_bytes(),
                            CancellationToken::new(),
                        )
                        .await
                        {
                            Ok(client) => Message::Profile(client),
                            Err(err) => Message::Error(ErrorDisplayInfo::new(
                                Arc::new(err),
                                t!("profile-picker.error.unlock"),
                            )),
                        }
                    }))
                } else {
                    Action::None
                }
            }
            Message::AddProfile(host) => {
                self.screen = Screen::AddProfile { host };

                Action::Run(iced::widget::operation::focus("host"))
            }
            Message::Connect(host) => {
                self.screen =
                    Screen::Loading(t!("profile-picker.connecting-to-server").to_string());
                Action::Run(Task::future(async move {
                    let connected = Client::first_connect(host).await;

                    match connected {
                        Ok(FirstConnect::Init(init)) => Message::Init(Arc::new(init)),
                        Ok(FirstConnect::Login(login)) => Message::Login(Arc::new(login)),
                        Err(e) => Message::Error(ErrorDisplayInfo::new(
                            Arc::new(e),
                            t!("profile-picker.error.connect-to-server"),
                        )),
                    }
                }))
            }
            Message::Init(init) => {
                let (state, task) = InitServer::start(init);
                self.screen = Screen::InitServer(state);

                Action::Run(task.map(Message::InitServer))
            }
            Message::Login(login) => {
                let (state, task) = LoginDialog::start(login);
                self.screen = Screen::LoginDialog(state);

                Action::Run(task.map(Message::LoginDialog))
            }
            Message::Profile(client) => Action::OpenProfile(client),
        }
    }

    fn add_profile(&mut self, host: String) {
        self.screen = Screen::AddProfile { host };
    }

    pub fn view(&self) -> Element<'_, Message> {
        let profiles = scrollable(
            button_list(self.profiles.iter().map(|profile| {
                button_list::entry(text(profile).size(20))
                    .on_press(Message::SelectProfile(profile.clone()))
            }))
            .entry_height(70),
        )
        .height(Length::Fill);

        let button_overlay = container(
            button(
                row![bootstrap::plus().size(30), text(t!("profile-picker.add"))]
                    .align_y(Vertical::Center)
                    .spacing(10)
                    .padding([0, 10]),
            )
            .on_press(Message::AddProfile(String::new())),
        )
        .align_bottom(Length::Fill)
        .align_right(Length::Fill)
        .padding(30);

        let overlay = match &self.screen {
            Screen::InitServer(init_server) => Some(init_server.view().map(Message::InitServer)),
            Screen::LoginDialog(login_dialog) => {
                Some(login_dialog.view().map(Message::LoginDialog))
            }
            Screen::Error(display_info) => Some(
                display_info
                    .view()
                    .on_close(Message::Reset)
                    .overlay()
                    .into(),
            ),
            Screen::Loading(message) => Some(loading(message).expand().into()),
            Screen::SelectProfile => None,
            Screen::UnlockProfile { profile, password } => Some(
                dialog(
                    column![
                        text(profile)
                            .align_x(Horizontal::Center)
                            .width(Length::Fill),
                        text_input(t!("generic.password"), password)
                            .id("password")
                            .secure(true)
                            .on_input(|input| Message::Input(Input::Password(input)))
                            .on_submit(Message::UnlockProfile),
                        space::vertical(),
                        button(text(t!("generic.delete")))
                            .on_press(Message::DeleteProfile(profile.clone()))
                            .style(button::danger),
                        button(text(t!("generic.unlock"))).on_press(Message::UnlockProfile)
                    ]
                    .spacing(20),
                )
                .title(text(t!("profile-picker.title.unlock")))
                .on_close(Message::Reset)
                .overlay()
                .into(),
            ),
            Screen::AddProfile { host } => Some(
                dialog(
                    column![
                        text_input(t!("generic.host"), host)
                            .id("host")
                            .on_input(|input| Message::Input(Input::Host(input)))
                            .on_submit(Message::Connect(host.clone())),
                        space::vertical(),
                        button(text(t!("generic.continue")))
                            .on_press(Message::Connect(host.clone()))
                    ]
                    .spacing(20),
                )
                .title(text(t!("profile-picker.title.add")))
                .on_close(Message::Reset)
                .overlay()
                .into(),
            ),
            Screen::ConfirmDelete(profile) => Some(
                dialog(
                    column![
                        text(t!("profile-picker.confirm-delete", "profile" => profile),),
                        space::vertical(),
                        button(text(t!("generic.delete")))
                            .on_press(Message::ConfirmDelete(profile.clone()))
                            .style(button::danger)
                    ]
                    .spacing(20),
                )
                .title(text(t!("profile-picker.title.delete")))
                .on_close(Message::SelectProfile(profile.clone()))
                .overlay()
                .into(),
            ),
        };
        stack![
            profiles,
            button_overlay,
            overlay.map(|element| opaque(element))
        ]
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
    }
}
