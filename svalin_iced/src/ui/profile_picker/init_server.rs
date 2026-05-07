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
    CopyTOTP,
    Continue,
    Back,
    Client(Arc<Client>),
    Username(String),
    Password(String),
    ConfirmPassword(String),
    Totp(String),
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
    username: String,
    password: String,
    confirm_password: String,
    totp_input: String,
}

enum State {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    Loading(String),
    User,
    Totp { totp: TOTP, qr: image::Handle },
}

impl InitServer {
    pub fn start(init: Arc<Init>) -> (Self, Task<Message>) {
        (
            Self {
                init: Some(init),
                state: State::User,
                username: String::new(),
                password: String::new(),
                confirm_password: String::new(),
                totp_input: String::new(),
            },
            iced::widget::operation::focus("username"),
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Error(display_info) => {
                self.error(display_info);

                Action::None
            }
            Message::Username(username) => {
                self.username = username;
                Action::None
            }
            Message::Password(password) => {
                self.password = password;
                Action::None
            }
            Message::ConfirmPassword(confirm_password) => {
                self.confirm_password = confirm_password;
                Action::None
            }
            Message::Totp(totp) => {
                self.totp_input = totp;
                Action::None
            }
            Message::CopyTOTP => {
                if let State::Totp { totp, .. } = &self.state {
                    Action::Run(iced::clipboard::write(totp.get_url()).discard())
                } else {
                    Action::None
                }
            }
            Message::Back => match &self.state {
                State::Loading(_) => Action::None,
                State::Totp { .. } => {
                    self.state = State::User;

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
                    State::User => {
                        // Todo: check inputs

                        if self.password != self.confirm_password {
                            return Action::None;
                        }

                        match new_totp(self.username.clone()) {
                            Err(err) => {
                                self.state = State::Error(ErrorDisplayInfo::new(
                                    Arc::new(err),
                                    t!("profile-picker.error.totp.register"),
                                ));

                                Action::None
                            }
                            Ok(totp) => {
                                let qr_code = image::Handle::from_bytes(totp.get_qr_png().unwrap());

                                self.state = State::Totp { totp, qr: qr_code };
                                self.totp_input.clear();

                                Action::Run(iced::widget::operation::focus("totp"))
                            }
                        }
                    }
                    State::Totp { totp, .. } => match totp.check_current(&self.totp_input) {
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
                                let username = self.username.clone();
                                let password = self.password.clone();
                                let totp = totp.clone();

                                self.state =
                                    State::Loading(t!("profile-picker.init-loading").to_string());

                                Action::Run(Task::future(async move {
                                    match init
                                        .init(username, password.clone().into_bytes(), totp)
                                        .await
                                    {
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

    pub fn view(&self) -> crate::Element<'_, Message> {
        match &self.state {
            State::Error(display_info) => display_info.view().on_close(Message::Back).into(),
            State::Loading(message) => loading(message).expand().into(),
            State::User => form()
                .title(t!("profile-picker.add"))
                .control(
                    text_input(&t!("generic.username"), &self.username)
                        .id("username")
                        .on_input(Message::Username),
                )
                .control(
                    text_input(&t!("generic.password"), &self.password)
                        .secure(true)
                        .on_input(Message::Password),
                )
                .control(
                    text_input(
                        &t!("profile-picker.input.confirm-password"),
                        &self.confirm_password,
                    )
                    .secure(true)
                    .on_input(Message::ConfirmPassword)
                    .on_submit_maybe(
                        if self.password == self.confirm_password {
                            Some(Message::Continue)
                        } else {
                            None
                        },
                    ),
                )
                .button(button(text(t!("generic.back"))).on_press(Message::Back))
                .button(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .into(),
            State::Totp { qr, .. } => form()
                .title(t!("profile-picker.add"))
                .control(image(qr))
                .control(button(text(t!("profile-picker.copy-totp"))).on_press(Message::CopyTOTP))
                .control(
                    text_input(&t!("profile-picker.input.totp"), &self.totp_input)
                        .id("totp")
                        .on_input(Message::Totp)
                        .on_submit(Message::Continue),
                )
                .button(button(text(t!("generic.back"))).on_press(Message::Back))
                .button(button(text(t!("generic.continue"))).on_press(Message::Continue))
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
