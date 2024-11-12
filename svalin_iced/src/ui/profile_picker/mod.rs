use std::sync::Arc;

use crate::fl;
use anyhow::Result;
use iced::{
    widget::{button, column, container, image, image::Handle, row, stack, text, text_input},
    Element, Length, Task,
};
use svalin::client::{Client, FirstConnect, Init, Login};

use super::widgets::{error_display, form, loading};

enum State {
    SelectProfile(Vec<String>),
    Error {
        context: String,
        error: Arc<anyhow::Error>,
    },
    UnlockProfile {
        profile: String,
        password: String,
    },
    Loading(String),
    AddProfile {
        host: String,
    },
    RegisterRoot(RegisterInfo),
    CreateTOTP {
        base: RegisterInfo,
        totp: totp_rs::TOTP,
        qr: Handle,
        totp_input: String,
    },
}

#[derive(Debug, Clone)]
struct RegisterInfo {
    init: Arc<Init>,
    username: String,
    password: String,
    confirm_password: String,
}

impl RegisterInfo {
    fn new(init: Init) -> Self {
        Self {
            init: Arc::new(init),
            username: String::new(),
            password: String::new(),
            confirm_password: String::new(),
        }
    }
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
    AddProfile(Option<String>),
    Connect(String),
    Init(RegisterInfo),
    RegisterTOTP(RegisterInfo),
    CopyTOTP(String),
    Login(Arc<Login>),
    Profile(Arc<Client>),
    Error {
        context: String,
        error: Arc<anyhow::Error>,
    },
}

#[derive(Debug, Clone)]
pub enum Input {
    Host(String),
    Username(String),
    Password(String),
    ConfirmPassword(String),
    TOTP(String),
}

impl Input {
    fn update(self, picker: &mut ProfilePicker) {
        match &mut picker.state {
            State::AddProfile { host } => {
                if let Input::Host(new_host) = self {
                    *host = new_host;
                }
            }
            State::RegisterRoot(RegisterInfo {
                init: _,
                username,
                password,
                confirm_password,
            }) => match self {
                Self::Username(new_username) => {
                    *username = new_username;
                }
                Self::ConfirmPassword(new_confirm_password) => {
                    *confirm_password = new_confirm_password;
                }
                Self::Password(new_password) => {
                    *password = new_password;
                }
                _ => unreachable!(),
            },
            State::CreateTOTP {
                base: _,
                totp: _,
                qr: _,
                totp_input,
            } => {
                if let Input::TOTP(new_totp_input) = self {
                    *totp_input = new_totp_input;
                }
            }
            _ => unreachable!(),
        }
    }
}

struct DeleteDialog {
    host: String,
    username: String,
    password: String,
    confirm_password: String,
}

impl ProfilePicker {
    pub fn new() -> Self {
        match Client::get_profiles() {
            Ok(profiles) => Self {
                state: State::SelectProfile(profiles),
                confirm_delete: None,
            },
            Err(err) => Self {
                state: State::Error {
                    context: fl!("profile-list-error"),
                    error: Arc::new(err),
                },
                confirm_delete: None,
            },
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Error { context, error } => self.state = State::Error { context, error },
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
                    self.state = State::Error {
                        context: fl!("profile-delete-error"),
                        error: Arc::new(error),
                    }
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
                            Err(err) => Message::Error {
                                context: fl!("profile-unlock-error"),
                                error: Arc::new(err),
                            },
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
                        Ok(FirstConnect::Init(init)) => Message::Init(RegisterInfo::new(init)),
                        Ok(FirstConnect::Login(login)) => Message::Login(Arc::new(login)),
                        Err(e) => Message::Error {
                            context: fl!("connect-to-server-error"),
                            error: Arc::new(e),
                        },
                    }
                });
            }
            Message::Init(init) => {
                self.state = State::RegisterRoot(init);
            }
            Message::RegisterTOTP(info) => match new_totp(info.username.clone()) {
                Ok(totp) => {
                    self.state = State::CreateTOTP {
                        base: info,
                        qr: Handle::from_bytes(totp.get_qr_png().unwrap()),
                        totp,
                        totp_input: String::new(),
                    };
                }
                Err(err) => {
                    self.state = State::Error {
                        context: fl!("register-totp-error"),
                        error: Arc::new(err),
                    };
                }
            },
            Message::CopyTOTP(totp) => {
                return iced::clipboard::write(totp);
            }
            Message::Login(login) => {
                todo!()
            }
            Message::Input(input) => {
                input.update(self);
            }
            Message::Profile(_) => unreachable!(),
        };

        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        match &self.state {
            State::Error { context, error } => error_display(error)
                .title(context)
                .on_close(Message::Reset)
                .into(),
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
            } => column![
                text(fl!("profile-unlock")),
                text_input("Password", password)
                    .on_input(|input| Message::Input(Input::Password(input)))
                    .on_submit(Message::UnlockProfile),
                row![
                    button(text("Cancel")).on_press(Message::Reset),
                    button(text("Unlock")).on_press(Message::UnlockProfile)
                ]
            ]
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
            State::RegisterRoot(info) => {
                let RegisterInfo {
                    init,
                    username,
                    password,
                    confirm_password,
                } = info;

                form()
                    .title(fl!("profile-add"))
                    .control(
                        column![
                            text_input(&fl!("username"), username)
                                .on_input(|input| Message::Input(Input::Username(input))),
                            text_input(&fl!("password"), password)
                                .on_input(|input| Message::Input(Input::Password(input))),
                            text_input(&fl!("confirm-password"), confirm_password)
                                .on_input(|input| Message::Input(Input::ConfirmPassword(input)))
                                .on_submit(Message::RegisterTOTP(info.clone())),
                        ]
                        .spacing(10),
                    )
                    .primary_action(
                        button(text(fl!("continue"))).on_press(Message::RegisterTOTP(info.clone())),
                    )
                    .secondary_action(
                        button(text(fl!("back")))
                            .on_press(Message::AddProfile(Some(init.address().to_string()))),
                    )
                    .into()
            }
            State::CreateTOTP {
                base,
                totp,
                totp_input,
                qr,
            } => form()
                .title(fl!("profile-add"))
                .control(
                    column![
                        image(qr),
                        button(text(fl!("copy-totp"))).on_press(Message::CopyTOTP(totp.get_url())),
                        text_input(&fl!("totp"), totp_input)
                            .on_input(|input| Message::Input(Input::TOTP(input)))
                            .on_submit(Message::Reset),
                    ]
                    .spacing(10),
                )
                .primary_action(button(text(fl!("continue"))).on_press(Message::Reset))
                .secondary_action(
                    button(text(fl!("back")))
                        .on_press(Message::AddProfile(Some(base.init.address().to_string()))),
                )
                .into(),
        }
    }

    pub fn dialog(&self) -> Option<Element<Message>> {
        if let Some(profile) = &self.confirm_delete {
            // return Some(
            // dialog()
            //     .body(format!("Are you sure you want to delete {}", profile))
            //     .primary_action(button(text("Cancel")).
            // on_press(Message::CalcelDelete))
            //     .secondary_action(
            //         button(text("Delete")).
            // on_press(Message::ConfirmDelete(profile.clone())),
            //     )
            //     .title("Delete Profile")
            //     .into(),
            // );
        }
        None
    }
}

pub fn new_totp(account_name: String) -> Result<totp_rs::TOTP> {
    Ok(totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1,
        8,
        1,
        30,
        totp_rs::Secret::generate_secret().to_bytes()?,
        Some("Svalin".into()),
        account_name,
    )?)
}
