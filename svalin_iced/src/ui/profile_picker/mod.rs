use std::sync::Arc;

use crate::Element;
use iced::{
    Length, Task,
    alignment::Vertical,
    padding,
    widget::{button, column, container, row, stack, text, text_input},
};
use init_server::InitServer;
use login::LoginDialog;
use svalin::client::{Client, FirstConnect, Init, Login};
use svalin_pki::sha2::digest::typenum::Le;

use super::{
    types::error_display_info::ErrorDisplayInfo,
    widgets::{dialog, form, icon, loading},
};

mod init_server;
mod login;

enum State {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    SelectProfile(Vec<String>),
    UnlockProfile { profile: String, password: String },
    Loading(String),
    AddProfile { host: String },
    InitServer(InitServer),
    LoginDialog(LoginDialog),
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
    CancelDelete,
    UnlockProfile,
    AddProfile(String),
    Connect(String),
    Init(Arc<Init>),
    InitServer(init_server::Message),
    LoginDialog(login::Message),
    Login(Arc<Login>),
    Profile(Arc<Client>),
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
        match &mut state.state {
            State::AddProfile { host } => {
                if let Input::Host(new_host) = self {
                    *host = new_host;
                }
            }
            State::UnlockProfile { password, .. } => {
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
                state: State::SelectProfile(Vec::new()),
                confirm_delete: None,
            },
            Task::done(Message::Reset),
        )
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::InitServer(message) => {
                if let State::InitServer(init_server) = &mut self.state {
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
                if let State::LoginDialog(login_dialog) = &mut self.state {
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
                self.state = State::Error(display_info);
                Action::None
            }
            Message::Input(input) => {
                input.update(self);

                Action::None
            }
            Message::Reset => {
                let profiles = Client::list_profiles().unwrap_or_else(|_| Vec::new());

                if profiles.is_empty() {
                    self.add_profile(String::new());
                } else {
                    self.state = State::SelectProfile(profiles);
                }

                Action::None
            }
            Message::SelectProfile(profile) => {
                self.state = State::UnlockProfile {
                    profile,
                    password: String::new(),
                };

                Action::Run(text_input::focus("password"))
            }
            Message::DeleteProfile(profile) => {
                self.confirm_delete = Some(profile.clone());
                Action::None
            }
            Message::ConfirmDelete(profile) => {
                self.confirm_delete = None;

                if let Err(error) = Client::remove_profile(&profile) {
                    self.state = State::Error(ErrorDisplayInfo::new(
                        Arc::new(error),
                        t!("profile-picker.error.delete"),
                    ));
                }

                Action::Run(Task::done(Message::Reset))
            }
            Message::CancelDelete => {
                self.confirm_delete = None;
                Action::None
            }
            Message::UnlockProfile => {
                if let State::UnlockProfile { profile, password } = &self.state {
                    let profile = profile.clone();
                    let password = password.clone();

                    self.state = State::Loading(t!("profile-picker.unlocking").to_string());

                    Action::Run(Task::future(async move {
                        match Client::open_profile_string(profile, password.into_bytes()).await {
                            Ok(client) => Message::Profile(Arc::new(client)),
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
                self.state = State::AddProfile { host };

                Action::Run(text_input::focus("host"))
            }
            Message::Connect(host) => {
                self.state = State::Loading(t!("profile-picker.connecting-to-server").to_string());
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
                self.state = State::InitServer(state);

                Action::Run(task.map(Message::InitServer))
            }
            Message::Login(login) => {
                let (state, task) = LoginDialog::start(login);
                self.state = State::LoginDialog(state);

                Action::Run(task.map(Message::LoginDialog))
            }
            Message::Profile(client) => Action::OpenProfile(client),
        }
    }

    fn add_profile(&mut self, host: String) {
        self.state = State::AddProfile { host };
    }

    pub fn view(&self) -> Element<Message> {
        match &self.state {
            State::InitServer(init_server) => init_server.view().map(Message::InitServer),
            State::LoginDialog(login_dialog) => login_dialog.view().map(Message::LoginDialog),
            State::Error(display_info) => display_info.view().on_close(Message::Reset).into(),
            State::Loading(message) => loading(message).expand().into(),
            State::SelectProfile(profiles) => {
                let profiles = column(profiles.iter().map(|p| {
                    row![
                        button(text(p).height(Length::Fill).align_y(Vertical::Center))
                            .on_press(Message::SelectProfile(p.clone()))
                            .width(Length::Fill)
                            .height(Length::Fill),
                        button(icon::delete().size(20).height(Length::Fill).center())
                            .on_press(Message::DeleteProfile(p.clone()))
                            .width(60)
                            .height(Length::Fill)
                    ]
                    .padding(10)
                    .spacing(10)
                    .height(80)
                    .into()
                }));

                let overlay = container(
                    button(
                        row![icon::add().size(30), text(t!("profile-picker.add"))]
                            .align_y(Vertical::Center)
                            .spacing(10)
                            .padding([0, 10]),
                    )
                    .on_press(Message::AddProfile(String::new())),
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
                .title(t!("profile-picker.title.unlock"))
                .control(
                    text_input(&t!("generic.password"), password)
                        .id("password")
                        .secure(true)
                        .on_input(|input| Message::Input(Input::Password(input)))
                        .on_submit(Message::UnlockProfile),
                )
                .primary_action(button(text(t!("generic.unlock"))).on_press(Message::UnlockProfile))
                .secondary_action(button(text(t!("generic.cancel"))).on_press(Message::Reset))
                .into(),
            State::AddProfile { host } => form()
                .title(t!("profile-picker.title.add"))
                .control(
                    text_input("Host", host)
                        .id("host")
                        .on_input(|input| Message::Input(Input::Host(input)))
                        .on_submit(Message::Connect(host.clone())),
                )
                .primary_action(
                    button(text(t!("generic.continue"))).on_press(Message::Connect(host.clone())),
                )
                .secondary_action(button(text(t!("generic.cancel"))).on_press(Message::Reset))
                .into(),
        }
    }

    pub fn dialog(&self) -> Option<Element<Message>> {
        if let Some(profile) = &self.confirm_delete {
            return Some(
                dialog()
                    .body(t!("profile-picker.confirm-delete", "profile" => profile))
                    .primary_action(button(text("Cancel")).on_press(Message::CancelDelete))
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
