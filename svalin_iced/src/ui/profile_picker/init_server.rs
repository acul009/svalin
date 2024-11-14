use std::sync::Arc;

use anyhow::Result;
use iced::{
    widget::{button, image, text, text_input},
    Task,
};
use svalin::client::Init;
use totp_rs::TOTP;

use crate::{
    fl,
    ui::{types::error_display_info::ErrorDisplayInfo, widgets::form},
};

pub struct InitServer {
    state: State,
    init: Arc<Init>,
}

enum State {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    User {
        username: String,
        password: String,
        confirm_password: String,
    },
    TOTP {
        username: String,
        password: String,
        totp: TOTP,
        qr: image::Handle,
        totp_input: String,
    },
}

#[derive(Debug, Clone)]
pub enum Message {
    Exit(String),
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    Input(Input),
    CopyTOTP,
    Continue,
    Back,
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
            State::TOTP {
                username,
                password,
                totp,
                qr,
                totp_input,
            } => match self {
                Self::Totp(new_totp) => *totp_input = new_totp,
                _ => (),
            },
            _ => (),
        }
    }
}

impl InitServer {
    pub fn new(init: Arc<Init>) -> Self {
        Self {
            init,
            state: State::User {
                username: String::new(),
                password: String::new(),
                confirm_password: String::new(),
            },
        }
    }
}

impl crate::ui::screen::SubScreen for InitServer {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> iced::Task<Self::Message> {
        match message {
            Message::Error(display_info) => self.state = State::Error(display_info),
            Message::Input(input) => input.update(self),
            Message::CopyTOTP => {
                if let State::TOTP { totp, .. } = &self.state {
                    return iced::clipboard::write(totp.get_url());
                }
            }
            Message::Back => match &self.state {
                State::TOTP {
                    username, password, ..
                } => {
                    self.state = State::User {
                        username: username.clone(),
                        password: password.clone(),
                        confirm_password: password.clone(),
                    }
                }
                State::Error(_) | State::User { .. } => {
                    return Task::done(Message::Exit(self.init.address().to_string()));
                }
            },
            Message::Continue => {
                match &self.state {
                    State::User {
                        username,
                        password,
                        confirm_password,
                    } => {
                        // Todo: check inputs

                        if password != confirm_password {
                            return Task::none();
                        }

                        match new_totp(username.clone()) {
                            Err(err) => {
                                self.state = State::Error(ErrorDisplayInfo::new(
                                    Arc::new(err),
                                    fl!("register-totp-error"),
                                ))
                            }
                            Ok(totp) => {
                                let qr_code = image::Handle::from_bytes(totp.get_qr_png().unwrap());

                                self.state = State::TOTP {
                                    username: username.clone(),
                                    password: password.clone(),
                                    totp,
                                    qr: qr_code,
                                    totp_input: String::new(),
                                };
                            }
                        }
                    }

                    _ => (),
                }
            }
            Message::Exit(_) => unreachable!(),
        }
        Task::none()
    }

    fn view(&self) -> crate::Element<Self::Message> {
        match &self.state {
            State::Error(display_info) => display_info.view().into(),
            State::User {
                username,
                password,
                confirm_password,
            } => form()
                .title(fl!("profile-add"))
                .control(
                    text_input(&fl!("username"), username)
                        .on_input(|input| Message::Input(Input::Username(input))),
                )
                .control(
                    text_input(&fl!("password"), password)
                        .secure(true)
                        .on_input(|input| Message::Input(Input::Password(input))),
                )
                .control(
                    text_input(&fl!("confirm-password"), confirm_password)
                        .secure(true)
                        .on_input(|input| Message::Input(Input::ConfirmPassword(input)))
                        .on_submit(Message::Continue),
                )
                .primary_action(button(text(fl!("continue"))).on_press(Message::Continue))
                .secondary_action(button(text(fl!("back"))).on_press(Message::Back))
                .into(),
            State::TOTP { qr, totp_input, .. } => form()
                .title(fl!("profile-add"))
                .control(image(qr))
                .control(button(text(fl!("copy-totp"))).on_press(Message::CopyTOTP))
                .control(
                    text_input(&fl!("totp"), totp_input)
                        .on_input(|input| Message::Input(Input::Totp(input))),
                )
                .primary_action(button(text(fl!("continue"))).on_press(Message::Continue))
                .secondary_action(button(text(fl!("back"))).on_press(Message::Back))
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
