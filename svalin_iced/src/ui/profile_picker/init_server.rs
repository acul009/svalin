use std::sync::Arc;

use anyhow::{Result, anyhow};
use iced::{
    Task,
    widget::{button, image, text, text_input},
};
use svalin::client::{Client, Init};
use totp_rs::TOTP;

use crate::ui::{
    types::error_display_info::ErrorDisplayInfo,
    widgets::{form, loading},
};

#[derive(Debug, Clone)]
pub enum Message {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    Input(Input),
    CopyTOTP,
    Continue,
    Back,
    Client(Arc<Client>),
}

pub enum Action {
    None,
    OpenProfile(Arc<Client>),
    Exit(String),
    Run(Task<Message>),
}

pub struct InitServer {
    state: State,
    init: Option<Arc<Init>>,
}

enum State {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    Loading(String),
    User {
        username: String,
        password: String,
        confirm_password: String,
    },
    Totp {
        username: String,
        password: String,
        totp: TOTP,
        qr: image::Handle,
        totp_input: String,
    },
}

#[derive(Debug, Clone)]
pub enum Input {
    Username(String),
    Password(String),
    ConfirmPassword(String),
    Totp(String),
}

impl Input {
    fn update(self, state: &mut InitServer) {
        match &mut state.state {
            State::User {
                username,
                password,
                confirm_password,
            } => match self {
                Self::Username(new_username) => *username = new_username,
                Self::Password(new_password) => *password = new_password,
                Self::ConfirmPassword(new_password) => *confirm_password = new_password,
                _ => (),
            },
            State::Totp { totp_input, .. } => {
                if let Self::Totp(new_totp) = self {
                    *totp_input = new_totp
                }
            }
            _ => (),
        }
    }
}

impl InitServer {
    pub fn start(init: Arc<Init>) -> (Self, Task<Message>) {
        (
            Self {
                init: Some(init),
                state: State::User {
                    username: String::new(),
                    password: String::new(),
                    confirm_password: String::new(),
                },
            },
            text_input::focus("username"),
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Error(display_info) => {
                self.error(display_info);

                Action::None
            }
            Message::Input(input) => {
                input.update(self);

                Action::None
            }
            Message::CopyTOTP => {
                if let State::Totp { totp, .. } = &self.state {
                    Action::Run(iced::clipboard::write(totp.get_url()))
                } else {
                    Action::None
                }
            }
            Message::Back => match &self.state {
                State::Loading(_) => Action::None,
                State::Totp {
                    username, password, ..
                } => {
                    self.state = State::User {
                        username: username.clone(),
                        password: password.clone(),
                        confirm_password: password.clone(),
                    };

                    Action::None
                }
                State::Error(_) | State::User { .. } => Action::Exit(
                    self.init
                        .as_ref()
                        .map_or("".to_string(), |init| init.address().to_string()),
                ),
            },
            Message::Continue => {
                match &self.state {
                    State::Loading(_) | State::Error(_) => Action::None,
                    State::User {
                        username,
                        password,
                        confirm_password,
                    } => {
                        // Todo: check inputs

                        if password != confirm_password {
                            return Action::None;
                        }

                        match new_totp(username.clone()) {
                            Err(err) => {
                                self.state = State::Error(ErrorDisplayInfo::new(
                                    Arc::new(err),
                                    t!("profile-picker.error.totp.register"),
                                ));

                                Action::None
                            }
                            Ok(totp) => {
                                let qr_code = image::Handle::from_bytes(totp.get_qr_png().unwrap());

                                self.state = State::Totp {
                                    username: username.clone(),
                                    password: password.clone(),
                                    totp,
                                    qr: qr_code,
                                    totp_input: String::new(),
                                };

                                Action::Run(text_input::focus("totp"))
                            }
                        }
                    }
                    State::Totp {
                        username,
                        password,
                        totp,
                        totp_input,
                        ..
                    } => match totp.check_current(totp_input) {
                        Err(err) => {
                            self.error(ErrorDisplayInfo::new(
                                Arc::new(err.into()),
                                t!("profile-picker.error.totp.verify"),
                            ));
                            return Action::None;
                        }
                        Ok(false) => {
                            self.error(ErrorDisplayInfo::new(
                                Arc::new(anyhow!("wrong totp")),
                                t!("profile-picker.error.totp.verify"),
                            ));
                            return Action::None;
                        }

                        Ok(true) => match self.init.take() {
                            None => {
                                self.error(ErrorDisplayInfo::new(
                                    Arc::new(anyhow!("init already used")),
                                    "init already used",
                                ));
                                return Action::None;
                            }
                            Some(init) => {
                                let init = match Arc::into_inner(init) {
                                    None => return Action::None,
                                    Some(init) => init,
                                };
                                let username = username.clone();
                                let password = password.clone();
                                let totp = totp.clone();

                                self.state =
                                    State::Loading(t!("profile-picker.init-loading").to_string());

                                Action::Run(Task::future(async move {
                                    match init.init(username, password.clone(), totp).await {
                                        Err(err) => Message::Error(ErrorDisplayInfo::new(
                                            Arc::new(err),
                                            t!("profile-picker.error.server-init"),
                                        )),
                                        Ok(profile) => {
                                            let client = match Client::open_profile(
                                                &profile,
                                                password.into(),
                                            )
                                            .await
                                            {
                                                Err(err) => {
                                                    return Message::Error(ErrorDisplayInfo::new(
                                                        Arc::new(err),
                                                        t!("profile-picker.error.server-init"),
                                                    ));
                                                }
                                                Ok(client) => client,
                                            };

                                            Message::Client(client)
                                        }
                                    }
                                }))
                            }
                        },
                    },
                }
            }
            Message::Client(client) => Action::OpenProfile(client),
        }
    }

    pub fn error(&mut self, display_info: ErrorDisplayInfo<Arc<anyhow::Error>>) {
        self.state = State::Error(display_info);
    }

    pub fn view(&self) -> crate::Element<Message> {
        match &self.state {
            State::Error(display_info) => display_info.view().on_close(Message::Back).into(),
            State::Loading(message) => loading(message).expand().into(),
            State::User {
                username,
                password,
                confirm_password,
            } => form()
                .title(t!("profile-picker.add"))
                .control(
                    text_input(&t!("generic.username"), username)
                        .id("username")
                        .on_input(|input| Message::Input(Input::Username(input))),
                )
                .control(
                    text_input(&t!("generic.password"), password)
                        .secure(true)
                        .on_input(|input| Message::Input(Input::Password(input))),
                )
                .control(
                    text_input(
                        &t!("profile-picker.input.confirm-password"),
                        confirm_password,
                    )
                    .secure(true)
                    .on_input(|input| Message::Input(Input::ConfirmPassword(input)))
                    .on_submit(Message::Continue),
                )
                .primary_action(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .secondary_action(button(text(t!("generic.back"))).on_press(Message::Back))
                .into(),
            State::Totp { qr, totp_input, .. } => form()
                .title(t!("profile-picker.add"))
                .control(image(qr))
                .control(button(text(t!("profile-picker.copy-totp"))).on_press(Message::CopyTOTP))
                .control(
                    text_input(&t!("profile-picker.input.totp"), totp_input)
                        .id("totp")
                        .on_input(|input| Message::Input(Input::Totp(input)))
                        .on_submit(Message::Continue),
                )
                .primary_action(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .secondary_action(button(text(t!("generic.back"))).on_press(Message::Back))
                .into(),
        }
    }
}

fn new_totp(account_name: String) -> Result<totp_rs::TOTP> {
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
