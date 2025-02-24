use std::sync::Arc;

use crate::Element;
use iced::{
    Length, Task,
    alignment::Vertical,
    widget::{button, column, container, row, stack, text, text_input},
};
use init_server::InitServer;
use svalin::client::{Client, FirstConnect, Init, Login};

use super::{
    action::Action,
    screen::SubScreen,
    types::error_display_info::ErrorDisplayInfo,
    widgets::{dialog, form, loading},
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
    CancelDelete,
    UnlockProfile,
    AddProfile(String),
    Connect(String),
    Init(Arc<Init>),
    InitServer(init_server::Message),
    Login(Arc<Login>),
    Profile(Arc<Client>),
}

pub enum Instruction {
    OpenProfile(Arc<Client>),
}

#[derive(Debug, Clone)]
pub enum Input {
    Host(String),
    Password(String),
}

impl Input {
    /// ***********  ✨ Codeium Command ⭐  ************
    /// Updates the state of the `ProfilePicker` based on the input provided.
    ///
    /// - If the current state is `State::AddProfile`, updates the host with the
    ///   new host value from the input.
    /// - If the current state is `State::UnlockProfile`, updates the password
    ///   with the new password value from the input.
    /// - Returns a `Task` that performs no additional actions.

    /// ****  90b3bd9a-7dc8-46fa-acd6-e8e465dfa8cd  ******
    fn update(self, state: &mut ProfilePicker) -> Task<Message> {
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
        Task::none()
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

    fn add_profile(&mut self, host: String) {
        self.state = State::AddProfile { host };
    }
}

impl SubScreen for ProfilePicker {
    type Message = Message;
    type Instruction = Instruction;

    fn update(&mut self, message: Message) -> Action<Instruction, Message> {
        match message {
            Message::InitServer(message) => {
                if let State::InitServer(init_server) = &mut self.state {
                    let mut action: Action<init_server::Instruction, Message> =
                        init_server.update(message).map(Message::InitServer);

                    if let Some(instruction) = action.instruction.take() {
                        match instruction {
                            init_server::Instruction::OpenProfile(client) => {
                                return action.with_instruction(Instruction::OpenProfile(client));
                            }
                            init_server::Instruction::Exit(host) => {
                                self.add_profile(host);
                            }
                        }
                    };

                    action.strip_instruction()
                } else {
                    Action::none()
                }
            }
            Message::Error(display_info) => {
                self.state = State::Error(display_info);
                Action::none()
            }
            Message::Input(input) => input.update(self).into(),
            Message::Reset => {
                let profiles = Client::get_profiles().unwrap_or_else(|_| Vec::new());

                if profiles.is_empty() {
                    self.add_profile(String::new());
                } else {
                    self.state = State::SelectProfile(profiles);
                }

                Action::none()
            }
            Message::SelectProfile(profile) => {
                self.state = State::UnlockProfile {
                    profile,
                    password: String::new(),
                };

                text_input::focus("password").into()
            }
            Message::DeleteProfile(profile) => {
                self.confirm_delete = Some(profile.clone());
                Action::none()
            }
            Message::ConfirmDelete(profile) => {
                self.confirm_delete = None;

                if let Err(error) = Client::remove_profile(&profile) {
                    self.state = State::Error(ErrorDisplayInfo::new(
                        Arc::new(error),
                        t!("profile-picker.error.delete"),
                    ));
                }

                Action::none()
            }
            Message::CancelDelete => {
                self.confirm_delete = None;
                Action::none()
            }
            Message::UnlockProfile => {
                if let State::UnlockProfile { profile, password } = &self.state {
                    let profile = profile.clone();
                    let password = password.clone();

                    self.state = State::Loading(t!("profile-picker.unlocking").to_string());

                    Task::future(async move {
                        match Client::open_profile_string(profile, password).await {
                            Ok(client) => Message::Profile(Arc::new(client)),
                            Err(err) => Message::Error(ErrorDisplayInfo::new(
                                Arc::new(err),
                                t!("profile-picker.error.unlock"),
                            )),
                        }
                    })
                    .into()
                } else {
                    Action::none()
                }
            }
            Message::AddProfile(host) => {
                self.state = State::AddProfile { host };

                text_input::focus("host").into()
            }
            Message::Connect(host) => {
                self.state = State::Loading(t!("profile-picker.connecting-to-server").to_string());
                Task::future(async move {
                    let connected = Client::first_connect(host).await;

                    match connected {
                        Ok(FirstConnect::Init(init)) => Message::Init(Arc::new(init)),
                        Ok(FirstConnect::Login(login)) => Message::Login(Arc::new(login)),
                        Err(e) => Message::Error(ErrorDisplayInfo::new(
                            Arc::new(e),
                            t!("profile-picker.error.connect-to-server"),
                        )),
                    }
                })
                .into()
            }
            Message::Init(init) => {
                let (state, task) = InitServer::start(init);
                self.state = State::InitServer(state);

                task.map(Message::InitServer).into()
            }
            Message::Login(_login) => {
                todo!()
            }
            Message::Profile(client) => Action::instruction(Instruction::OpenProfile(client)),
        }
    }

    fn view(&self) -> Element<Message> {
        match &self.state {
            State::InitServer(init_server) => init_server.view().map(Message::InitServer),
            State::Error(display_info) => display_info.view().on_close(Message::Reset).into(),
            State::Loading(message) => loading(message).expand().into(),
            State::SelectProfile(profiles) => {
                let profiles = column(profiles.iter().map(|p| {
                    row![
                        button(text(p).height(Length::Fill).align_y(Vertical::Center))
                            .on_press(Message::SelectProfile(p.clone()))
                            .width(Length::Fill)
                            .height(Length::Fill),
                        button(text("🗑️").height(Length::Fill).center())
                            .on_press(Message::DeleteProfile(p.clone()))
                            .width(50)
                            .height(Length::Fill)
                    ]
                    .height(60)
                    .into()
                }));

                let overlay = container(
                    button(text(t!("profile-picker.add")))
                        .padding(10)
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

    fn dialog(&self) -> Option<Element<Message>> {
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
